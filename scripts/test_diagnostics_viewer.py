import importlib.util
import json
import sys
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).with_name("diagnostics-viewer.py")
SPEC = importlib.util.spec_from_file_location("diagnostics_viewer", SCRIPT_PATH)
diagnostics_viewer = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = diagnostics_viewer
SPEC.loader.exec_module(diagnostics_viewer)


class ViewerApp(diagnostics_viewer.App):
    def compose(self):
        yield diagnostics_viewer.FileViewer()


class FakeSession:
    def __init__(self, files):
        self.files = files

    def read_file(self, filename):
        return self.files.get(filename)


class MatchSpanTests(unittest.TestCase):
    def test_unicode_case_insensitive_matches_use_original_text_offsets(self):
        text = "İstanbul i"

        matches = diagnostics_viewer.find_matches(text, "i")

        self.assertEqual(matches, [(0, 1), (9, 10)])
        highlighted = diagnostics_viewer.highlight_matches(text, matches, 0)
        self.assertEqual(highlighted.plain, text)
        self.assertEqual(len(highlighted), len(text))


class FileViewerSearchTests(unittest.IsolatedAsyncioTestCase):
    async def test_plain_search_can_close_and_reopen(self):
        app = ViewerApp()
        async with app.run_test(size=(100, 40)) as pilot:
            file_viewer = app.query_one(diagnostics_viewer.FileViewer)
            file_viewer.update_content(FakeSession({"plain.txt": "first needle second"}), "plain.txt")
            await pilot.pause()

            text_widget = file_viewer.search_text_widget
            file_viewer._search("needle")
            self.assertEqual(file_viewer.search_matches, [(6, 12)])

            file_viewer._close_search()
            await pilot.pause()
            self.assertIs(file_viewer.search_text_widget, text_widget)
            self.assertEqual(file_viewer.search_matches, [])
            self.assertEqual(text_widget.render().plain, "first needle second")
            self.assertEqual(text_widget.render().spans, [])
            results = file_viewer.query_one("#search-results", diagnostics_viewer.Static)
            self.assertEqual(results.render().plain, "No matches")

            file_viewer.action_search()
            file_viewer._search("needle")
            self.assertEqual(file_viewer.search_matches, [(6, 12)])

    async def test_json_search_reaches_values_beyond_the_old_depth_limit(self):
        data = {"target": "deep needle"}
        for depth in range(12):
            data = {f"level-{depth}": data}

        app = ViewerApp()
        async with app.run_test(size=(100, 40)) as pilot:
            file_viewer = app.query_one(diagnostics_viewer.FileViewer)
            file_viewer.update_content(FakeSession({"deep.json": json.dumps(data)}), "deep.json")
            await pilot.pause()

            file_viewer._search("needle")

            self.assertEqual(len(file_viewer.search_nodes), 1)
            tree = file_viewer.query_one(diagnostics_viewer.JsonTreeView)
            path, value = file_viewer.search_nodes[0]
            self.assertIn("needle", tree.reveal_match(path, value).label.plain)

    async def test_json_search_is_iterative_near_the_parser_depth_limit(self):
        depth = 800
        content = '{"level":' * depth + '"deep needle"' + "}" * depth

        app = ViewerApp()
        async with app.run_test(size=(100, 40)) as pilot:
            file_viewer = app.query_one(diagnostics_viewer.FileViewer)
            file_viewer.update_content(FakeSession({"deep.json": content}), "deep.json")
            await pilot.pause()

            file_viewer._search("needle")

            self.assertEqual(len(file_viewer.search_nodes), 1)
            tree = file_viewer.query_one(diagnostics_viewer.JsonTreeView)
            path, value = file_viewer.search_nodes[0]
            self.assertIn("needle", tree.reveal_match(path, value).label.plain)
            self.assertLessEqual(tree.rendered_node_count, tree.MAX_TOTAL_NODES)

    async def test_wide_json_rendering_is_bounded_but_omitted_values_are_searchable(self):
        data = {
            f"key-{index}": f"value-{index}"
            for index in range(diagnostics_viewer.JsonTreeView.MAX_INITIAL_NODES + 100)
        }
        data["target"] = "wide needle"

        app = ViewerApp()
        async with app.run_test(size=(100, 40)) as pilot:
            file_viewer = app.query_one(diagnostics_viewer.FileViewer)
            file_viewer.update_content(FakeSession({"wide.json": json.dumps(data)}), "wide.json")
            await pilot.pause()

            tree = file_viewer.query_one(diagnostics_viewer.JsonTreeView)
            self.assertLessEqual(len(tree.root.children), tree.MAX_INITIAL_NODES + 1)

            file_viewer._search("needle")

            self.assertEqual(len(file_viewer.search_nodes), 1)
            path, value = file_viewer.search_nodes[0]
            self.assertIn("needle", tree.reveal_match(path, value).label.plain)
            self.assertLessEqual(tree.rendered_node_count, tree.MAX_TOTAL_NODES)

    async def test_json_search_reports_visit_limits_instead_of_false_no_matches(self):
        data = {f"key-{index}": f"value-{index}" for index in range(6)}
        data["key-5"] = "needle"

        app = ViewerApp()
        async with app.run_test(size=(100, 40)) as pilot:
            file_viewer = app.query_one(diagnostics_viewer.FileViewer)
            file_viewer.update_content(FakeSession({"limited.json": json.dumps(data)}), "limited.json")
            await pilot.pause()

            tree = file_viewer.query_one(diagnostics_viewer.JsonTreeView)
            tree.MAX_SEARCH_VISITS = 5
            file_viewer._search("needle")

            self.assertEqual(file_viewer.search_nodes, [])
            self.assertEqual(file_viewer.search_limit, "visits")
            results = file_viewer.query_one("#search-results", diagnostics_viewer.Static)
            self.assertEqual(results.render().plain, "No matches before search limit")

    async def test_json_search_uses_json_null_spelling(self):
        data = {"value": None}
        for depth in range(12):
            data = {f"level-{depth}": data}

        app = ViewerApp()
        async with app.run_test(size=(100, 40)) as pilot:
            file_viewer = app.query_one(diagnostics_viewer.FileViewer)
            file_viewer.update_content(FakeSession({"null.json": json.dumps(data)}), "null.json")
            await pilot.pause()

            file_viewer._search("null")

            self.assertEqual(len(file_viewer.search_nodes), 1)
            tree = file_viewer.query_one(diagnostics_viewer.JsonTreeView)
            path, value = file_viewer.search_nodes[0]
            self.assertIsNone(value)
            self.assertIn("null", tree.reveal_match(path, value).label.plain)

    async def test_omitted_search_proxy_displays_rich_markup_as_literal_text(self):
        key = "[red]path[/red][/]["
        value = "needle [red]value[/red] [/]["
        data = {key: value}
        for depth in range(12):
            data = {f"safe-{depth}": data}

        app = ViewerApp()
        async with app.run_test(size=(100, 40)) as pilot:
            file_viewer = app.query_one(diagnostics_viewer.FileViewer)
            file_viewer.update_content(FakeSession({"markup.json": json.dumps(data)}), "markup.json")
            await pilot.pause()

            file_viewer._search("needle")

            tree = file_viewer.query_one(diagnostics_viewer.JsonTreeView)
            self.assertIn(key, tree.cursor_node.label.plain)
            self.assertIn(value, tree.cursor_node.label.plain)

    async def test_omitted_long_string_search_proxy_opens_full_value(self):
        value = "needle " + "complete-value-" * 20
        data = {"target": value}
        for depth in range(12):
            data = {f"safe-{depth}": data}

        class SelectedEvent:
            def __init__(self, node):
                self.node = node
                self.stopped = False

            def stop(self):
                self.stopped = True

        app = ViewerApp()
        async with app.run_test(size=(100, 40)) as pilot:
            file_viewer = app.query_one(diagnostics_viewer.FileViewer)
            file_viewer.update_content(FakeSession({"long.json": json.dumps(data)}), "long.json")
            await pilot.pause()

            file_viewer._search("needle")
            tree = file_viewer.query_one(diagnostics_viewer.JsonTreeView)
            event = SelectedEvent(tree.cursor_node)
            tree.on_tree_node_selected(event)
            await pilot.pause()

            self.assertTrue(event.stopped)
            self.assertIsInstance(app.screen, diagnostics_viewer.TextViewerModal)
            self.assertEqual(app.screen.text, value)

    async def test_shallow_json_labels_display_rich_markup_as_literal_text(self):
        data = {
            "[/]": "[/]",
            "[red]key[/red]": "[red]value[/red]",
            "[": "[",
        }

        app = ViewerApp()
        async with app.run_test(size=(100, 40)) as pilot:
            file_viewer = app.query_one(diagnostics_viewer.FileViewer)
            file_viewer.update_content(FakeSession({"markup.json": json.dumps(data)}), "markup.json")
            await pilot.pause()

            tree = file_viewer.query_one(diagnostics_viewer.JsonTreeView)
            for key, value in data.items():
                self.assertEqual(tree.nodes_by_path[(key,)].label.plain, f'{key}: "{value}"')

    async def test_one_proxy_represents_many_omitted_matches_across_searches(self):
        data = {}
        for index in range(150):
            value = f"needle-{index}"
            for depth in range(20):
                value = {f"level-{depth}": value}
            data[f"branch-{index}"] = value

        app = ViewerApp()
        async with app.run_test(size=(100, 40)) as pilot:
            file_viewer = app.query_one(diagnostics_viewer.FileViewer)
            file_viewer.update_content(FakeSession({"many.json": json.dumps(data)}), "many.json")
            await pilot.pause()

            file_viewer._search("needle")
            tree = file_viewer.query_one(diagnostics_viewer.JsonTreeView)
            self.assertEqual(len(file_viewer.search_nodes), tree.MAX_SEARCH_RESULTS)
            results = file_viewer.query_one("#search-results", diagnostics_viewer.Static)
            self.assertEqual(results.render().plain, "1/100+")

            for index, (path, value) in enumerate(file_viewer.search_nodes):
                file_viewer.search_index = index
                file_viewer._show_tree_match(tree)
                self.assertEqual(tree.cursor_node.data["path"], path)
                self.assertIn(str(value), tree.cursor_node.label.plain)

            self.assertLessEqual(tree.rendered_node_count, tree.MAX_TOTAL_NODES)

            file_viewer._search("needle-149")
            await pilot.pause()
            self.assertEqual(len(file_viewer.search_nodes), 1)
            path, value = file_viewer.search_nodes[0]
            self.assertEqual(tree.cursor_node.data["path"], path)
            self.assertIn(str(value), tree.cursor_node.label.plain)
            self.assertLessEqual(tree.rendered_node_count, tree.MAX_TOTAL_NODES)
            self.assertEqual(results.render().plain, "1/1")


if __name__ == "__main__":
    unittest.main()
