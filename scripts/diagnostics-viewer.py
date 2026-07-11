#!/usr/bin/env -S uv run --quiet --script
# /// script
# dependencies = ["textual>=0.87.0", "pyperclip"]
# ///
"""
WARNING: entirely vibe coded. use as a throwaway tool


Diagnostics Viewer - Browse and inspect Gosling diagnostics reports.

Scans for diagnostics JSON reports and legacy zip files, displays their sessions, and provides
an interactive viewer for examining session data, logs, and other files.
"""
import json
import re
import sys
import zipfile
from pathlib import Path
from typing import Optional, Any

import pyperclip
from rich.text import Text

from textual.app import App, ComposeResult
from textual.widgets import Header, Footer, Static, Tree, ListView, ListItem, Label, Input
from textual.containers import Horizontal, Vertical, VerticalScroll, Container
from textual.binding import Binding
from textual.message import Message
from textual.screen import ModalScreen


def truncate_string(s: str, max_len: int = 100, edge_len: int = 35) -> str:
    """Truncate a string if it's longer than max_len."""
    if len(s) <= max_len:
        return s

    omitted = len(s) - (2 * edge_len)
    return f"{s[:edge_len]}[{omitted} more]{s[-edge_len:]}"


def find_matches(text: str, query: str) -> list[tuple[int, int]]:
    if not query:
        return []

    return [match.span() for match in re.finditer(re.escape(query), text, re.IGNORECASE)]


def highlight_matches(text: str, matches: list[tuple[int, int]], selected: int) -> Text:
    highlighted = Text(text)
    for index, (start, end) in enumerate(matches):
        style = "black on yellow" if index == selected else "bold yellow"
        highlighted.stylize(style, start, end)
    return highlighted


class JsonTreeView(Tree):
    """A tree widget for displaying collapsible JSON."""

    MAX_RENDER_DEPTH = 10
    MAX_INITIAL_NODES = 2_000
    MAX_TOTAL_NODES = MAX_INITIAL_NODES + 1
    MAX_SEARCH_RESULTS = 100
    MAX_SEARCH_VISITS = 100_000

    BINDINGS = [
        Binding("ctrl+o", "toggle_all", "Toggle All", show=True),
    ]

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.json_data = None
        self.show_root = False
        self.all_expanded = False
        self.nodes_by_path = {}
        self.rendered_node_count = 0
        self.search_proxy = None

    def load_json(self, data: Any, label: str = "JSON"):
        """Load JSON data into the tree."""
        self.json_data = data
        self.clear()
        self.root.label = label
        self.nodes_by_path = {(): self.root}
        self.rendered_node_count = 0
        self.search_proxy = None
        self._build_tree(self.root, data)
        self.root.expand()

    def action_toggle_all(self):
        """Toggle expansion of all nodes."""
        self.all_expanded = not self.all_expanded
        if self.all_expanded:
            self.root.expand_all()
        else:
            self.root.collapse_all()
            self.root.expand()  # Keep root expanded

    def on_tree_node_selected(self, event: Tree.NodeSelected):
        """Handle node selection - show modal for truncated strings."""
        node = event.node

        # Check if this is a truncated string node
        if node.data and isinstance(node.data, dict) and node.data.get("truncated"):
            key = node.data["key"]
            value = node.data["value"]

            # Show the full string in a modal
            title = f"Full String Value for '{key}'"
            self.app.push_screen(TextViewerModal(title, value))

            # Prevent default tree expansion behavior
            event.stop()

    def _add_value_node(self, parent, key, value, path, parent_is_list, expand=False):
        label = Text(str(key), style="yellow" if parent_is_list else "cyan")
        label.append(": ")
        if isinstance(value, (dict, list)) and value:
            suffix = "{...}" if isinstance(value, dict) else "[...]"
            label.append(suffix)
            child = parent.add(label, expand=expand)
            child.data = {
                "key": key,
                "value": value,
                "type": type(value).__name__,
                "path": path,
            }
        elif isinstance(value, str):
            truncated = truncate_string(value)
            label.append(f'"{truncated}"', style="green")
            if truncated != value:
                child = parent.add(label, expand=False)
                child.data = {
                    "key": key,
                    "value": value,
                    "type": "str",
                    "path": path,
                    "truncated": True,
                }
                child.allow_expand = False
            else:
                child = parent.add_leaf(label)
        elif isinstance(value, bool):
            label.append(str(value).lower(), style="magenta")
            child = parent.add_leaf(label)
        elif isinstance(value, (int, float)):
            label.append(str(value), style="yellow")
            child = parent.add_leaf(label)
        elif value is None:
            label.append("null", style="dim")
            child = parent.add_leaf(label)
        else:
            label.append(str(value))
            child = parent.add_leaf(label)

        self.nodes_by_path[path] = child
        self.rendered_node_count += 1
        return child

    def _build_tree(self, node, data, current_depth=0, path=()):
        """Build a bounded initial tree; deep search paths are revealed on demand."""
        if not isinstance(data, (dict, list)):
            return

        items = data.items() if isinstance(data, dict) else enumerate(data)
        for key, value in items:
            if self.rendered_node_count >= self.MAX_INITIAL_NODES:
                return
            if self.rendered_node_count == self.MAX_INITIAL_NODES - 1:
                node.add_leaf("[dim]... additional entries omitted[/dim]")
                self.rendered_node_count += 1
                return

            child_path = path + (key,)
            child = self._add_value_node(
                node,
                key,
                value,
                child_path,
                isinstance(data, list),
                expand=(current_depth == 0),
            )
            if not isinstance(value, (dict, list)) or not value:
                continue
            if current_depth >= self.MAX_RENDER_DEPTH:
                if self.rendered_node_count < self.MAX_INITIAL_NODES:
                    child.add_leaf("[dim]... deeper entries omitted[/dim]")
                    self.rendered_node_count += 1
                continue
            self._build_tree(child, value, current_depth + 1, child_path)

    @staticmethod
    def _iter_items(data):
        return iter(data.items()) if isinstance(data, dict) else iter(enumerate(data))

    @staticmethod
    def _search_value_text(value):
        return value if isinstance(value, str) else json.dumps(value, ensure_ascii=False)

    def find_json_matches(self, query: str):
        """Search JSON iteratively without materializing the full tree."""
        if not query or not isinstance(self.json_data, (dict, list)):
            return [], None

        matches = []
        visits = 0
        stack = [((), self._iter_items(self.json_data))]
        while stack:
            parent_path, items = stack[-1]
            try:
                key, value = next(items)
            except StopIteration:
                stack.pop()
                continue

            if visits >= self.MAX_SEARCH_VISITS:
                return matches, "visits"
            visits += 1
            path = parent_path + (key,)
            is_container = isinstance(value, (dict, list)) and bool(value)
            value_text = "" if is_container else self._search_value_text(value)
            if find_matches(str(key), query) or find_matches(value_text, query):
                matches.append((path, value))
                if len(matches) > self.MAX_SEARCH_RESULTS:
                    return matches[: self.MAX_SEARCH_RESULTS], "results"
            if is_container:
                stack.append((path, self._iter_items(value)))

        return matches, None

    def reveal_match(self, path, matched_value):
        """Select a rendered node or reuse one labeled proxy for an omitted match."""
        existing = self.nodes_by_path.get(path)
        if existing is not None:
            return existing

        display_path = truncate_string(
            " / ".join(str(part) for part in path),
            max_len=160,
            edge_len=60,
        )
        if isinstance(matched_value, dict):
            preview = "{...}"
        elif isinstance(matched_value, list):
            preview = "[...]"
        else:
            preview = truncate_string(
                self._search_value_text(matched_value), max_len=120, edge_len=45
            )
        label = Text("Search match", style="bold yellow")
        label.append(f" {display_path}: {preview}")

        if self.search_proxy is None:
            self.search_proxy = self.root.add_leaf(label)
            self.rendered_node_count += 1
        else:
            self.search_proxy.set_label(label)
        self.search_proxy.data = {
            "key": path[-1] if path else "match",
            "path": path,
            "value": matched_value,
            "search_proxy": True,
            "truncated": (
                isinstance(matched_value, str)
                and truncate_string(matched_value) != matched_value
            ),
        }
        return self.search_proxy

    def clear_search_proxy(self):
        if self.search_proxy is not None:
            self.search_proxy.remove()
            self.search_proxy = None
            self.rendered_node_count -= 1


class TextViewerModal(ModalScreen):
    """Modal screen for viewing long text strings."""

    BINDINGS = [
        Binding("escape,q,enter", "dismiss", "Close", show=True),
        Binding("c", "copy", "Copy", show=True),
    ]

    def __init__(self, title: str, text: str):
        super().__init__()
        self.title = title
        self.text = text

    def compose(self) -> ComposeResult:
        """Compose the modal content."""
        with Vertical(id="modal-container"):
            yield Static(Text(self.title, style="bold"), id="modal-title")
            with VerticalScroll(id="modal-scroll"):
                yield Static(Text(self.text), id="modal-text")
            yield Static("[dim]Press C to copy, Escape/Q/Enter to close[/dim]", id="modal-footer")

    def action_dismiss(self):
        """Dismiss the modal."""
        self.app.pop_screen()

    def action_copy(self):
        """Copy the text to clipboard."""
        pyperclip.copy(self.text)
        self.notify("Copied to clipboard")


class SearchOverlay(Container):
    """Search overlay widget."""

    BINDINGS = [
        Binding("enter", "next_match", "Next", show=True),
        Binding("shift+enter", "previous_match", "Previous", show=True),
        Binding("escape", "close", "Close", show=True),
    ]

    class QueryChanged(Message):
        def __init__(self, query: str):
            super().__init__()
            self.query = query

    class NextMatch(Message):
        pass

    class PreviousMatch(Message):
        pass

    class Close(Message):
        pass

    def __init__(self):
        super().__init__()
        self.display = False

    def compose(self) -> ComposeResult:
        with Horizontal(id="search-container"):
            yield Static("Search: ", id="search-label")
            yield Input(placeholder="Type to search...", id="search-input")
            yield Static("", id="search-results")

    def on_input_changed(self, event: Input.Changed):
        self.post_message(self.QueryChanged(event.value))

    def on_input_submitted(self, event: Input.Submitted):
        self.post_message(self.NextMatch())

    def on_key(self, event):
        if event.key == "shift+enter":
            self.post_message(self.PreviousMatch())
            event.stop()

    def action_next_match(self):
        self.post_message(self.NextMatch())

    def action_previous_match(self):
        self.post_message(self.PreviousMatch())

    def action_close(self):
        self.post_message(self.Close())

    def set_results(self, selected: int, total: int, limit=None):
        results = self.query_one("#search-results", Static)
        if limit == "results":
            label = f"{selected + 1}/{total}+"
        elif limit == "visits":
            label = f"{selected + 1}/{total} (partial)" if total else "No matches before search limit"
        else:
            label = f"{selected + 1}/{total}" if total else "No matches"
        results.update(label)


class DiagnosticsSession:
    """Represents a diagnostics report or legacy diagnostics bundle."""

    def __init__(self, path: Path):
        self.path = path
        self.is_zip = path.suffix == ".zip"
        self.name = "Unknown Session"
        self.session_id = path.stem
        self.created_at = path.stat().st_mtime
        self.report = None
        self._load_session_name()

    def _load_session_name(self):
        """Extract session name from the report."""
        if not self.is_zip:
            self._load_json_report()
            session = (self.report or {}).get("session") or {}
            self.name = session.get("name", "Unknown Session")
            self.session_id = session.get("id", self.path.stem)
            return

        try:
            with zipfile.ZipFile(self.path, 'r') as zf:
                # Find session.json
                for name in zf.namelist():
                    if name.endswith('session.json'):
                        with zf.open(name) as f:
                            data = json.load(f)
                            self.name = data.get('name', 'Unknown Session')
                            self.session_id = data.get('id', self.path.stem)
                        break
        except Exception as e:
            self.name = f"Error loading: {e}"

    def _load_json_report(self):
        if self.report is not None:
            return

        try:
            self.report = json.loads(self.path.read_text())
        except Exception as e:
            self.report = {"error": f"Error loading: {e}"}

    def _json_virtual_files(self) -> dict[str, str]:
        self._load_json_report()
        report = self.report or {}
        files = {
            "diagnostics.json": json.dumps(report, indent=2),
        }

        for key, filename in [
            ("system", "system.json"),
            ("config", "config.json"),
            ("extensions", "extensions.json"),
            ("session", "session.json"),
            ("schedule", "schedule.json"),
            ("errors", "errors.json"),
        ]:
            value = report.get(key)
            if value is not None:
                files[filename] = json.dumps(value, indent=2)

        logs = report.get("logs") or {}
        server = logs.get("server")
        if isinstance(server, dict) and server.get("content") is not None:
            files["logs/server.txt"] = server["content"]

        llm_logs = logs.get("llm") or []
        for index, entry in enumerate(llm_logs):
            if isinstance(entry, dict) and entry.get("content") is not None:
                path = Path(entry.get("path") or f"llm_request.{index}.jsonl")
                files[f"logs/{path.name}"] = entry["content"]

        config = report.get("config") or {}
        if isinstance(config, dict) and config.get("configYaml"):
            files["config.yaml"] = config["configYaml"]

        for prompt in report.get("prompts") or []:
            if isinstance(prompt, dict) and prompt.get("name") and prompt.get("content") is not None:
                files[f"prompts/{prompt['name']}.txt"] = prompt["content"]

        return files

    def get_file_list(self) -> list[str]:
        """Get list of report files, sorted with system first."""
        if not self.is_zip:
            files = list(self._json_virtual_files().keys())

            def sort_key(f):
                if f == "system.json":
                    return (0, f)
                elif f == "session.json":
                    return (1, f)
                elif f == "config.yaml" or f == "config.json":
                    return (2, f)
                elif f == "diagnostics.json":
                    return (3, f)
                else:
                    return (4, f)

            return sorted(files, key=sort_key)

        try:
            with zipfile.ZipFile(self.path, 'r') as zf:
                files = zf.namelist()

                # Sort: system.txt first, then session.json, then alphabetically
                def sort_key(f):
                    if f.endswith('system.txt'):
                        return (0, f)
                    elif f.endswith('session.json'):
                        return (1, f)
                    elif f.endswith('config.yaml'):
                        return (2, f)
                    else:
                        return (3, f)

                return sorted(files, key=sort_key)
        except Exception:
            return []

    def read_file(self, filename: str) -> Optional[str]:
        """Read a file from the report.

        Returns:
            File content as string, or None if file cannot be read.
        """
        if not self.is_zip:
            return self._json_virtual_files().get(filename)

        try:
            with zipfile.ZipFile(self.path, 'r') as zf:
                with zf.open(filename) as f:
                    return f.read().decode('utf-8', errors='replace')
        except Exception:
            # File not found or cannot be read
            return None


class FileContentPane(Vertical):
    """A pane that shows either JSON tree or plain text."""

    def __init__(self, title: str):
        super().__init__()
        self.title = title
        self.content_type = "empty"
        self.json_data = None
        self.text_content = ""

    def compose(self) -> ComposeResult:
        """Compose the pane content."""
        if self.content_type == "json":
            tree = JsonTreeView(self.title)
            if self.json_data is not None:
                tree.load_json(self.json_data, self.title)
            yield tree
        elif self.content_type == "text":
            with VerticalScroll():
                yield Static(self.text_content)
        else:
            yield Static("[dim]No content[/dim]")

    def set_json(self, data: Any):
        """Set JSON content."""
        self.content_type = "json"
        self.json_data = data

    def set_text(self, text: str):
        """Set text content."""
        self.content_type = "text"
        self.text_content = text


class FileViewer(Vertical):
    """Widget for viewing file contents."""

    def __init__(self):
        super().__init__()
        self.current_session = None
        self.current_filename = None
        self.current_part = None
        self.search_text = ""
        self.search_matches = []
        self.search_nodes = []
        self.search_limit = None
        self.search_index = 0
        self.search_text_widget = None
        self.search_scroll = None

    def compose(self) -> ComposeResult:
        """Create child widgets."""
        with Vertical(id="content-area"):
            yield Static("[dim]Select a file to view[/dim]")

        yield SearchOverlay()

    def update_content(self, session: DiagnosticsSession, filename: str, part: str = None):
        """Update the viewer with new file content.

        Args:
            session: The diagnostics session
            filename: The file to display
            part: For JSONL files, either "request" or "responses"
        """
        self.current_session = session
        self.current_filename = filename
        self.current_part = part
        self._reset_search_for_content()

        content = session.read_file(filename)
        if content is None:
            self._show_plain(filename, f"[red]Error: Could not read file '{filename}'[/red]")
            return

        # Check if this is a JSONL file
        if filename.endswith('.jsonl') and part:
            self._show_jsonl(filename, content, part)
        elif filename.endswith('.json'):
            self._show_json(filename, content)
        else:
            self._show_plain(filename, content)

        # Auto-focus the content
        self.post_message(self.ContentReady())

    def _show_jsonl(self, filename: str, content: str, part: str):
        """Show JSONL file - either request or responses part."""
        lines = [line.strip() for line in content.strip().split('\n') if line.strip()]

        # Parse lines
        request_data = None
        responses = []

        if len(lines) > 0:
            try:
                request_data = json.loads(lines[0])
            except (json.JSONDecodeError, RecursionError):
                # Skip malformed request line; diagnostics may be truncated or corrupted
                pass

        for i in range(1, len(lines)):
            try:
                responses.append(json.loads(lines[i]))
            except (json.JSONDecodeError, RecursionError):
                # Skip individual malformed response lines; show only valid JSON entries
                pass

        # Show content
        content_area = self.query_one("#content-area", Vertical)
        content_area.remove_children()

        if part == "request" and request_data:
            tree = JsonTreeView(f"{filename} - request")
            tree.load_json(request_data, f"{filename} - request")
            content_area.mount(tree)
            self.search_text = ""
        elif part == "responses" and responses:
            tree = JsonTreeView(f"{filename} - responses")
            if len(responses) == 1:
                tree.load_json(responses[0], f"{filename} - response")
            else:
                tree.load_json(responses, f"{filename} - responses")
            self.search_text = ""
            content_area.mount(tree)
        else:
            content_area.mount(Static("[red]No data available for this part[/red]"))
            self.search_text = ""

    def _show_json(self, filename: str, content: str):
        """Show JSON file with collapsible tree."""
        # Show content
        content_area = self.query_one("#content-area", Vertical)
        content_area.remove_children()

        tree = JsonTreeView(filename)
        try:
            data = json.loads(content)
            tree.load_json(data, filename)
            self.search_text = ""
        except (json.JSONDecodeError, RecursionError) as e:
            tree.root.add_leaf(f"[red]Error parsing JSON: {e}[/red]")
            self.search_text = content

        content_area.mount(tree)

    def _show_plain(self, filename: str, content: str):
        """Show plain text content."""
        # Show content
        content_area = self.query_one("#content-area", Vertical)
        content_area.remove_children()

        # Create and mount the scroll container with the content
        scroll = VerticalScroll()
        content_area.mount(scroll)
        self.search_text = content
        self.search_scroll = scroll
        self.search_text_widget = Static(Text(content))
        scroll.mount(self.search_text_widget)

    def focus_content(self):
        """Focus the content area."""
        try:
            # Try to focus a tree if present
            tree = self.query_one(JsonTreeView)
            tree.focus()
        except Exception:
            # No JsonTreeView present (e.g., showing plain text), which is fine
            pass

    def action_search(self):
        overlay = self.query_one(SearchOverlay)
        if overlay.display:
            self._close_search()
            return

        overlay.display = True
        overlay.query_one("#search-input", Input).focus()

    def on_search_overlay_query_changed(self, event: SearchOverlay.QueryChanged):
        self._search(event.query)

    def on_search_overlay_next_match(self, event: SearchOverlay.NextMatch):
        self._move_search(1)

    def on_search_overlay_previous_match(self, event: SearchOverlay.PreviousMatch):
        self._move_search(-1)

    def on_search_overlay_close(self, event: SearchOverlay.Close):
        self._close_search()

    def _clear_search_results(self):
        self.search_matches = []
        self.search_nodes = []
        self.search_limit = None
        self.search_index = 0

    def _restore_plain_text(self):
        if self.search_text_widget is not None:
            self.search_text_widget.update(Text(self.search_text))

    def _clear_tree_search_proxy(self):
        try:
            self.query_one(JsonTreeView).clear_search_proxy()
        except Exception:
            pass

    def _clear_search_content(self):
        self._clear_search_results()
        self.search_text_widget = None
        self.search_scroll = None

    def _reset_search_for_content(self):
        self._restore_plain_text()
        self._clear_tree_search_proxy()
        overlay = self.query_one(SearchOverlay)
        overlay.display = False
        overlay.query_one("#search-input", Input).value = ""
        self._clear_search_content()
        overlay.set_results(0, 0)

    def _close_search(self):
        self._restore_plain_text()
        self._clear_tree_search_proxy()
        overlay = self.query_one(SearchOverlay)
        overlay.display = False
        overlay.query_one("#search-input", Input).value = ""
        self._clear_search_results()
        overlay.set_results(0, 0)
        self.focus_content()

    def _search(self, query: str):
        self.search_matches = []
        self.search_nodes = []
        self.search_index = 0

        if not query:
            self._clear_tree_search_proxy()
            if self.search_text_widget is not None:
                self.search_text_widget.update(Text(self.search_text))
            self.query_one(SearchOverlay).set_results(0, 0)
            return

        if self.search_text_widget is not None:
            self.search_matches = find_matches(self.search_text, query)
            self._show_text_match(query)
            return

        try:
            tree = self.query_one(JsonTreeView)
        except Exception:
            self.query_one(SearchOverlay).set_results(0, 0)
            return

        tree.clear_search_proxy()
        self.search_nodes, self.search_limit = tree.find_json_matches(query)
        self._show_tree_match(tree)

    def _move_search(self, direction: int):
        if self.search_matches:
            self.search_index = (self.search_index + direction) % len(self.search_matches)
            self._show_text_match(self.query_one("#search-input", Input).value)
        elif self.search_nodes:
            self.search_index = (self.search_index + direction) % len(self.search_nodes)
            self._show_tree_match(self.query_one(JsonTreeView))

    def _show_text_match(self, query: str):
        overlay = self.query_one(SearchOverlay)
        overlay.set_results(self.search_index, len(self.search_matches))
        if not self.search_matches:
            return

        self.search_text_widget.update(
            highlight_matches(self.search_text, self.search_matches, self.search_index)
        )
        match_start, _ = self.search_matches[self.search_index]
        line = self.search_text[:match_start].count("\n")
        self.search_scroll.scroll_to(y=line, animate=False)

    def _show_tree_match(self, tree: JsonTreeView):
        overlay = self.query_one(SearchOverlay)
        overlay.set_results(self.search_index, len(self.search_nodes), self.search_limit)
        if not self.search_nodes:
            return

        path, matched_value = self.search_nodes[self.search_index]
        node = tree.reveal_match(path, matched_value)
        parent = node.parent
        while parent is not None:
            parent.expand()
            parent = parent.parent
        tree.move_cursor(node)
        tree.select_node(node)
        tree.scroll_to_node(node, animate=False)

    class ContentReady(Message):
        """Message sent when content is ready to be focused."""
        pass


class SessionViewer(Vertical):
    """Widget for viewing a diagnostics session."""

    BINDINGS = [
        Binding("ctrl+f,cmd+f", "search", "Search", show=True),
        Binding("c", "copy_file", "Copy file", show=True),
    ]

    def __init__(self, session: DiagnosticsSession):
        super().__init__()
        self.session = session

    def compose(self) -> ComposeResult:
        """Create child widgets."""
        yield Static(f"[bold yellow]Session: {self.session.name}[/bold yellow]", id="session-title")

        with Horizontal(id="main-content"):
            # Left side: File browser
            with Vertical(id="file-browser"):
                yield Static("[bold]Files:[/bold]")
                tree = Tree("Files", id="file-tree")
                tree.show_root = False

                # Build file tree
                files = self.session.get_file_list()

                # Group by directory
                dirs = {}
                for file in files:
                    parts = file.split('/')
                    is_jsonl = file.endswith('.jsonl')

                    if len(parts) == 1:
                        # Root file
                        if is_jsonl:
                            # Add two entries for JSONL files
                            tree.root.add_leaf(f"{file} - request", data={"file": file, "part": "request"})
                            tree.root.add_leaf(f"{file} - responses", data={"file": file, "part": "responses"})
                        else:
                            tree.root.add_leaf(file, data={"file": file, "part": None})
                    else:
                        # File in directory
                        dir_name = parts[0]
                        if dir_name not in dirs:
                            dirs[dir_name] = tree.root.add(dir_name, expand=True)

                        file_name = '/'.join(parts[1:])
                        if is_jsonl:
                            # Add two entries for JSONL files
                            dirs[dir_name].add_leaf(f"{file_name} - request", data={"file": file, "part": "request"})
                            dirs[dir_name].add_leaf(f"{file_name} - responses", data={"file": file, "part": "responses"})
                        else:
                            dirs[dir_name].add_leaf(file_name, data={"file": file, "part": None})

                yield tree

            # Right side: File viewer
            yield FileViewer()

    def on_mount(self):
        """Handle mount event."""
        # Show system.txt by default and select it in tree
        files = self.session.get_file_list()
        system_file = next((f for f in files if f.endswith('system.txt')), None)
        if system_file:
            viewer = self.query_one(FileViewer)
            viewer.update_content(self.session, system_file)

            # Select the first node in the tree
            tree = self.query_one("#file-tree", Tree)
            if tree.root.children:
                tree.select_node(tree.root.children[0])

        # Focus the tree initially
        tree = self.query_one("#file-tree", Tree)
        tree.focus()

    def on_tree_node_selected(self, event: Tree.NodeSelected):
        """Handle file selection."""
        # Only handle selections from the file tree, not the JSON tree
        if event.control.id != "file-tree":
            return

        # Make sure it's a file (has dict data), not a directory
        if event.node.data and isinstance(event.node.data, dict) and event.node.parent:
            viewer = self.query_one(FileViewer)
            file_path = event.node.data["file"]
            part = event.node.data["part"]
            viewer.update_content(self.session, file_path, part)

    def on_file_viewer_content_ready(self, event: FileViewer.ContentReady):
        """Handle content ready event by focusing the viewer."""
        viewer = self.query_one(FileViewer)
        viewer.focus_content()

    def action_search(self):
        """Toggle search in the file viewer."""
        viewer = self.query_one(FileViewer)
        viewer.action_search()

    def action_copy_file(self):
        """Copy the current file content to clipboard."""
        viewer = self.query_one(FileViewer)
        if not viewer.current_session or not viewer.current_filename:
            self.app.notify("No file selected")
            return

        content = viewer.current_session.read_file(viewer.current_filename)
        if content is None:
            self.app.notify("Could not read file")
            return

        # For JSONL files with a part, extract just that part and pretty-format
        if viewer.current_filename.endswith('.jsonl') and viewer.current_part:
            lines = [line.strip() for line in content.strip().split('\n') if line.strip()]
            if viewer.current_part == "request" and lines:
                try:
                    data = json.loads(lines[0])
                    content = json.dumps(data, indent=2)
                except json.JSONDecodeError:
                    content = lines[0]
            elif viewer.current_part == "responses" and len(lines) > 1:
                try:
                    responses = [json.loads(line) for line in lines[1:]]
                    if len(responses) == 1:
                        content = json.dumps(responses[0], indent=2)
                    else:
                        content = json.dumps(responses, indent=2)
                except json.JSONDecodeError:
                    content = '\n'.join(lines[1:])
        # Pretty-format regular JSON files too
        elif viewer.current_filename.endswith('.json'):
            try:
                data = json.loads(content)
                content = json.dumps(data, indent=2)
            except json.JSONDecodeError:
                pass

        pyperclip.copy(content)
        self.app.notify("Copied to clipboard")

    def on_key(self, event):
        """Handle left/right navigation between panels."""
        if event.key == "left":
            tree = self.query_one("#file-tree", Tree)
            tree.focus()
        elif event.key == "right":
            viewer = self.query_one(FileViewer)
            viewer.focus_content()


class SessionList(Vertical):
    """Widget for listing available sessions."""

    def __init__(self, sessions: list[DiagnosticsSession]):
        super().__init__()
        self.sessions = sessions

    def compose(self) -> ComposeResult:
        """Create child widgets."""
        yield Static("[bold yellow]Available Diagnostics Sessions[/bold yellow]\n")

        if not self.sessions:
            yield Static("[red]No diagnostics files found[/red]")
        else:
            yield Static(f"[dim]Found {len(self.sessions)} session(s)[/dim]\n")
            yield ListView(id="session-list")

    def on_mount(self):
        """Populate the list after mounting."""
        list_view = self.query_one(ListView)
        for session in self.sessions:
            item = ListItem(
                Label(f"{session.name}\n[dim]{session.path.name}[/dim]"),
                name=session.path.name
            )
            list_view.append(item)


class DiagnosticsApp(App):
    """Diagnostics viewer application."""

    # Disable command palette (Ctrl+\)
    ENABLE_COMMAND_PALETTE = False

    CSS = """
    Screen {
        background: $surface;
    }

    /* Modal styles */
    TextViewerModal {
        align: center middle;
    }

    #modal-container {
        width: 80%;
        height: 80%;
        background: $surface;
        border: thick $primary;
        padding: 1;
    }

    #modal-title {
        background: $primary;
        color: $text;
        padding: 1;
        text-align: center;
        dock: top;
    }

    #modal-scroll {
        height: 1fr;
        border: solid $accent;
        padding: 1;
        margin: 1 0;
    }

    #modal-text {
        width: 100%;
    }

    #modal-footer {
        text-align: center;
        dock: bottom;
    }

    #session-title {
        padding: 1;
        background: $primary;
        color: $text;
        text-align: center;
        height: 3;
    }

    #main-content {
        height: 100%;
    }

    #file-browser {
        width: 30%;
        border-right: solid $primary;
        padding: 1;
    }

    FileViewer {
        width: 70%;
        height: 100%;
    }

    #content-area {
        height: 100%;
        padding: 1;
    }

    JsonTreeView {
        height: 100%;
        scrollbar-gutter: stable;
    }

    #search-container {
        height: 3;
        background: $panel;
        padding: 1;
        border-top: solid $primary;
    }

    #search-label {
        width: auto;
        margin-right: 1;
    }

    #search-input {
        width: 1fr;
        margin-right: 1;
    }

    #search-results {
        width: auto;
    }

    SearchOverlay {
        height: auto;
    }

    #session-list {
        height: 100%;
    }

    ListView {
        background: $surface;
    }

    ListItem {
        padding: 1;
    }

    ListItem:hover {
        background: $primary 30%;
    }

    Tree {
        height: 100%;
    }

    Tree:focus {
        border: solid $accent;
    }
    """

    BINDINGS = [
        Binding("q", "quit", "Quit"),
        Binding("escape", "back", "Back to list"),
        Binding("ctrl+f,cmd+f", "search", "Search", show=False),
    ]

    def __init__(self, diagnostics_dir: Path):
        super().__init__()
        self.diagnostics_dir = diagnostics_dir
        self.sessions = []
        self.current_view = None

    def compose(self) -> ComposeResult:
        """Create child widgets."""
        yield Header()
        yield Footer()

    def on_mount(self):
        """Handle mount event."""
        self.title = "Gosling Diagnostics Viewer"
        self.scan_diagnostics()
        self.show_session_list()

    def scan_diagnostics(self):
        """Scan for diagnostics JSON reports and legacy zip files."""
        self.sessions = []

        for path in [
            *self.diagnostics_dir.glob("diagnostics*.json"),
            *self.diagnostics_dir.glob("diagnostics*.zip"),
        ]:
            self.sessions.append(DiagnosticsSession(path))

        # Sort by creation time (newest first)
        self.sessions.sort(key=lambda s: s.created_at, reverse=True)

    def show_session_list(self):
        """Show the session list view."""
        if self.current_view:
            self.current_view.remove()

        self.current_view = SessionList(self.sessions)
        self.mount(self.current_view)

    def show_session_viewer(self, session: DiagnosticsSession):
        """Show the session viewer."""
        if self.current_view:
            self.current_view.remove()

        self.current_view = SessionViewer(session)
        self.mount(self.current_view)

    def on_list_view_selected(self, event: ListView.Selected):
        """Handle session selection."""
        # Find the session by diagnostics file name
        session_name = event.item.name
        session = next((s for s in self.sessions if s.path.name == session_name), None)
        if session:
            self.show_session_viewer(session)

    def action_back(self):
        """Go back to session list."""
        if isinstance(self.current_view, SessionViewer):
            self.show_session_list()

    def action_quit(self):
        """Quit the application."""
        self.exit()

    def action_search(self):
        """Toggle search."""
        if isinstance(self.current_view, SessionViewer):
            self.current_view.action_search()


def main():
    """Main entry point."""
    # Get diagnostics directory from args or use default
    if len(sys.argv) > 1:
        diagnostics_dir = Path(sys.argv[1]).expanduser()
    else:
        diagnostics_dir = Path.home() / "Downloads"

    if not diagnostics_dir.exists():
        print(f"Error: Directory '{diagnostics_dir}' not found", file=sys.stderr)
        sys.exit(1)

    app = DiagnosticsApp(diagnostics_dir)
    app.run()


if __name__ == "__main__":
    main()
