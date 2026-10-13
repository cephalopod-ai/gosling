import React, { useState } from "react";
import { Box, Text, useInput } from "ink";
import type {
  RequestPermissionRequest,
  RequestPermissionResponse,
} from "@agentclientprotocol/sdk";
import { GOLD, TEAL, TEXT_DIM, TEXT_PRIMARY } from "../colors.js";
import { truncateTerminalText, wrapTerminalText } from "../utils.js";

// ARC-GOS-001: the ACP client's `requestPermission` handler is the seam
// where the protocol relay meets the human's actual decision. This renders
// the pending request and blocks on real keyboard input rather than
// silently answering it on the user's behalf.

const KIND_LABELS: Record<string, string> = {
  allow_once: "Allow once",
  allow_always: "Allow always for this tool",
  reject_once: "Reject once",
  reject_always: "Reject always for this tool",
};

// Scoped grants (a domain or folder) reuse a standard kind under a distinct
// id, so only an option whose id is its kind gets the tool-wide kind label.
function optionLabel(
  kind: string | undefined,
  optionId: string,
  name: string,
): string {
  if (kind && optionId === kind && KIND_LABELS[kind]) return KIND_LABELS[kind]!;
  return name || optionId;
}

function serializeRawInput(rawInput: unknown): string | undefined {
  if (rawInput == null) return undefined;
  let json: string;
  try {
    json = JSON.stringify(rawInput);
  } catch {
    return undefined;
  }
  if (!json || json === "{}") return undefined;
  return json;
}

export function permissionPromptLayout(
  height: number,
  optionCount: number,
  hasInput: boolean,
  selected: number,
) {
  const available = Math.max(height - 4, 0);
  const minimumInputRows = hasInput && available >= 3 ? 2 : 0;
  const showFooter = available >= optionCount + minimumInputRows + 2;
  const optionBudget = Math.max(
    available - minimumInputRows - (showFooter ? 2 : 0),
    0,
  );
  const showOptionIndicator = optionCount > optionBudget && optionBudget >= 2;
  const optionRows = Math.min(
    optionCount,
    optionBudget - (showOptionIndicator ? 1 : 0),
  );
  const optionStart = Math.max(
    0,
    Math.min(selected - Math.floor(optionRows / 2), optionCount - optionRows),
  );
  const payloadLineBudget =
    minimumInputRows > 0
      ? available -
        optionRows -
        (showOptionIndicator ? 1 : 0) -
        (showFooter ? 2 : 0) -
        1
      : 0;
  return {
    showFooter,
    showOptionIndicator,
    optionRows,
    optionStart,
    payloadLineBudget,
  };
}

export function PermissionPrompt({
  request,
  width,
  height,
  onRespond,
}: {
  request: RequestPermissionRequest;
  width: number;
  height: number;
  onRespond: (response: RequestPermissionResponse) => void;
}) {
  const options = request.options;
  // Never default the cursor to an "allow always"-shaped option: the user
  // must actively navigate to it. `allow_once` is the least-persistent
  // affirmative choice, so it is the safest default cursor position when
  // present; otherwise fall back to the first option in the list.
  const initialIndex = Math.max(
    options.findIndex((option) => option.kind === "allow_once"),
    0,
  );
  const [selected, setSelected] = useState(initialIndex);
  const [payloadScroll, setPayloadScroll] = useState(0);

  const boxWidth = Math.max(width, 6);
  const boxHeight = Math.max(height, 4);
  const contentWidth = Math.max(boxWidth - 4, 1);
  const toolName = request.toolCall.title ?? request.toolCall.toolCallId;
  const rawInput = serializeRawInput(request.toolCall.rawInput);
  const payloadLines = rawInput ? wrapTerminalText(rawInput, contentWidth) : [];
  const {
    showFooter,
    showOptionIndicator,
    optionRows,
    optionStart,
    payloadLineBudget,
  } = permissionPromptLayout(
    boxHeight,
    options.length,
    Boolean(rawInput),
    selected,
  );
  const visibleOptions = options.slice(optionStart, optionStart + optionRows);
  const maxPayloadScroll = Math.max(payloadLines.length - payloadLineBudget, 0);
  const visiblePayloadLines = payloadLines.slice(
    Math.min(payloadScroll, maxPayloadScroll),
    Math.min(payloadScroll, maxPayloadScroll) + payloadLineBudget,
  );

  useInput((input, key) => {
    if (options.length === 0) return;
    if (key.pageUp || (key.ctrl && input === "u")) {
      setPayloadScroll((offset) =>
        Math.max(Math.min(offset, maxPayloadScroll) - payloadLineBudget, 0),
      );
    } else if (key.pageDown || (key.ctrl && input === "d")) {
      setPayloadScroll((offset) =>
        Math.min(offset + payloadLineBudget, maxPayloadScroll),
      );
    } else if (key.upArrow) {
      setSelected((i) => (i - 1 + options.length) % options.length);
    } else if (key.downArrow) {
      setSelected((i) => (i + 1) % options.length);
    } else if (key.return && optionRows > 0) {
      const option = options[selected];
      if (option) {
        onRespond({
          outcome: { outcome: "selected", optionId: option.optionId },
        });
      }
    } else if (key.escape) {
      onRespond({ outcome: { outcome: "cancelled" } });
    }
  });

  return (
    <Box
      flexDirection="column"
      width={boxWidth}
      height={boxHeight}
      borderStyle="round"
      borderColor={GOLD}
      paddingX={1}
    >
      <Text bold color={GOLD} wrap="truncate">
        {truncateTerminalText("Permission requested", contentWidth)}
      </Text>
      <Text wrap="truncate" color={TEXT_PRIMARY}>
        {truncateTerminalText(toolName, contentWidth)}
      </Text>
      {rawInput && payloadLineBudget > 0 ? (
        <Box flexDirection="column">
          <Text color={TEXT_DIM} wrap="truncate">
            {truncateTerminalText(
              `Input ${Math.min(payloadScroll, maxPayloadScroll) + 1}-${Math.min(
                Math.min(payloadScroll, maxPayloadScroll) + payloadLineBudget,
                payloadLines.length,
              )} of ${payloadLines.length}`,
              contentWidth,
            )}
          </Text>
          {visiblePayloadLines.map((line, index) => (
            <Text
              key={`${payloadScroll}-${index}`}
              color={TEXT_DIM}
              wrap="truncate"
            >
              {line}
            </Text>
          ))}
        </Box>
      ) : null}
      <Box flexDirection="column">
        {showOptionIndicator ? (
          <Text color={TEXT_DIM} wrap="truncate">
            {truncateTerminalText(
              `Choices ${optionStart + 1}-${optionStart + optionRows} of ${options.length}`,
              contentWidth,
            )}
          </Text>
        ) : null}
        {visibleOptions.map((option, offset) => (
          <Text
            key={option.optionId}
            color={optionStart + offset === selected ? TEAL : TEXT_DIM}
            wrap="truncate"
          >
            {optionStart + offset === selected ? "> " : "  "}
            {truncateTerminalText(
              optionLabel(option.kind, option.optionId, option.name),
              contentWidth - 2,
            )}
          </Text>
        ))}
      </Box>
      {showFooter ? (
        <Box marginTop={1}>
          <Text color={TEXT_DIM} wrap="truncate">
            {truncateTerminalText(
              rawInput && maxPayloadScroll > 0
                ? "↑↓ choose · pgup/pgdn inspect input · enter confirm · esc cancel"
                : "↑↓ choose · enter confirm · esc cancel",
              contentWidth,
            )}
          </Text>
        </Box>
      ) : null}
    </Box>
  );
}
