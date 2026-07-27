import React, { useState } from "react";
import { Box, Text, useInput } from "ink";
import type {
  RequestPermissionRequest,
  RequestPermissionResponse,
} from "@agentclientprotocol/sdk";
import { GOLD, TEAL, TEXT_DIM, TEXT_PRIMARY } from "../colors.js";

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

function summarizeRawInput(rawInput: unknown, maxLen: number): string | undefined {
  if (rawInput == null) return undefined;
  let json: string;
  try {
    json = JSON.stringify(rawInput);
  } catch {
    return undefined;
  }
  if (!json || json === "{}") return undefined;
  return json.length > maxLen
    ? `${json.slice(0, Math.max(maxLen - 1, 0))}…`
    : json;
}

export function PermissionPrompt({
  request,
  width,
  onRespond,
}: {
  request: RequestPermissionRequest;
  width: number;
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

  useInput((_input, key) => {
    if (options.length === 0) return;
    if (key.upArrow) {
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

  const boxWidth = Math.max(width, 20);
  const toolName = request.toolCall.title ?? request.toolCall.toolCallId;
  const inputSummary = summarizeRawInput(request.toolCall.rawInput, boxWidth - 4);

  return (
    <Box
      flexDirection="column"
      width={boxWidth}
      borderStyle="round"
      borderColor={GOLD}
      paddingX={1}
    >
      <Text bold color={GOLD}>
        Permission requested
      </Text>
      <Text wrap="truncate" color={TEXT_PRIMARY}>
        {toolName}
      </Text>
      {inputSummary ? (
        <Text wrap="truncate" color={TEXT_DIM}>
          {inputSummary}
        </Text>
      ) : null}
      <Box flexDirection="column" marginTop={1}>
        {options.map((option, index) => (
          <Text
            key={option.optionId}
            color={index === selected ? TEAL : TEXT_DIM}
            wrap="truncate"
          >
            {index === selected ? "> " : "  "}
            {optionLabel(option.kind, option.optionId)}
          </Text>
        ))}
      </Box>
      <Box marginTop={1}>
        <Text color={TEXT_DIM}>↑↓ choose · enter confirm · esc cancel</Text>
      </Box>
    </Box>
  );
}
