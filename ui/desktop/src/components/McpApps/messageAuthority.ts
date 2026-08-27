const MESSAGE_PREVIEW_LIMIT = 1_000;

export function confirmMcpAppMessage(
  extensionName: string,
  text: string,
  confirm: (message: string) => boolean = window.confirm
): boolean {
  const appName = extensionName || 'MCP App';
  const preview =
    text.length > MESSAGE_PREVIEW_LIMIT
      ? `${text.slice(0, MESSAGE_PREVIEW_LIMIT)}\n\n… (${text.length - MESSAGE_PREVIEW_LIMIT} more characters)`
      : text;
  return confirm(
    `${appName} wants to send the following message as you. Review it before allowing it into the conversation:\n\n${preview}`
  );
}
