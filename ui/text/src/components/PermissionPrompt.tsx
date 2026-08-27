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

function optionLabel(kind: string | undefined, optionId: string): string {
  if (kind && KIND_LABELS[kind]) return KIND_LABELS[kind]!;
  return optionId;
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

  const boxWidth = Math.max(width, 20);
  const boxHeight = Math.max(height, 10);
  const contentWidth = Math.max(boxWidth - 4, 1);
  const toolName = request.toolCall.title ?? request.toolCall.toolCallId;
  const rawInput = serializeRawInput(request.toolCall.rawInput);
  const payloadLines = rawInput ? wrapTerminalText(rawInput, contentWidth) : [];
  const baseLines = 2 + 1 + 1 + options.length + (rawInput ? 1 : 0);
  const showFooter = boxHeight >= baseLines + (rawInput ? 1 : 0) + 2;
  const showOptionsMargin =
    boxHeight >= baseLines + (rawInput ? 1 : 0) + (showFooter ? 2 : 0) + 1;
  const payloadLineBudget = rawInput
    ? Math.max(
        1,
        boxHeight -
          baseLines -
          (showFooter ? 2 : 0) -
          (showOptionsMargin ? 1 : 0),
      )
    : 0;
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
    } else if (key.return) {
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
      <Text bold color={GOLD}>
        Permission requested
      </Text>
      <Text wrap="truncate" color={TEXT_PRIMARY}>
        {truncateTerminalText(toolName, contentWidth)}
      </Text>
      {rawInput ? (
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
      <Box flexDirection="column" marginTop={showOptionsMargin ? 1 : 0}>
        {options.map((option, index) => (
          <Text
            key={option.optionId}
            color={index === selected ? TEAL : TEXT_DIM}
            wrap="truncate"
          >
            {index === selected ? "> " : "  "}
            {truncateTerminalText(
              optionLabel(option.kind, option.optionId),
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
