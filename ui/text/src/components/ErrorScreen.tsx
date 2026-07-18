import React from "react";
import { Box, Text, useInput } from "ink";
import { CRANBERRY, TEXT_PRIMARY, TEXT_DIM } from "../colors.js";
import { truncateTerminalText } from "../utils.js";

interface ErrorScreenProps {
  errorMsg: string;
  width: number;
  onRetry: () => void;
}

export const ErrorScreen = React.memo(function ErrorScreen({
  errorMsg,
  width,
  onRetry,
}: ErrorScreenProps) {
  useInput((_ch, key) => {
    if (key.return || key.escape) {
      onRetry();
    }
  });

  const maxWidth = Math.max(1, Math.min(width - 4, 80));
  const contentWidth = Math.max(1, maxWidth - 4);
  const visibleError = truncateTerminalText(errorMsg, contentWidth);

  return (
    <Box flexDirection="column" paddingX={2} width={maxWidth}>
      <Text color={CRANBERRY} bold wrap="truncate">
        {truncateTerminalText("✗ Setup error", contentWidth)}
      </Text>
      {visibleError && (
        <Box width={contentWidth}>
          <Text color={TEXT_PRIMARY} wrap="truncate">
            {visibleError}
          </Text>
        </Box>
      )}
      <Box marginTop={1} width={contentWidth}>
        <Text color={TEXT_DIM} wrap="truncate">
          {truncateTerminalText("press enter to retry", contentWidth)}
        </Text>
      </Box>
    </Box>
  );
});
