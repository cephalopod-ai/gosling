import React, { useState } from 'react';
import { Copy } from './icons';
import { defineMessages, useIntl } from '../i18n';
import { writeRichTextToClipboard, writeTextToClipboard } from '../utils/clipboard';

const i18n = defineMessages({
  copied: {
    id: 'messageCopyLink.copied',
    defaultMessage: 'Copied!',
  },
  copy: {
    id: 'messageCopyLink.copy',
    defaultMessage: 'Copy',
  },
});

interface MessageCopyLinkProps {
  text: string;
  contentRef: React.RefObject<HTMLDivElement | null>;
}

function getHtmlContent(contentRef: React.RefObject<HTMLDivElement | null>): string | null {
  if (!contentRef.current) {
    return null;
  }

  const container = document.createElement('div');
  container.innerHTML = contentRef.current.innerHTML;
  container.querySelectorAll('button').forEach((button) => button.remove());

  const html = container.innerHTML.trim();
  return html.length > 0 ? html : null;
}

export default function MessageCopyLink({ text, contentRef }: MessageCopyLinkProps) {
  const intl = useIntl();
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      const html = getHtmlContent(contentRef);
      if (html) {
        await writeRichTextToClipboard(html, text);
      } else {
        await writeTextToClipboard(text);
      }

      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy text: ', err);
    }
  };

  return (
    <button
      type="button"
      onClick={handleCopy}
      className="flex font-mono items-center gap-1 text-xs text-text-secondary hover:cursor-pointer hover:text-text-primary transition-all duration-200 opacity-0 group-hover:opacity-100 -translate-y-4 group-hover:translate-y-0"
    >
      <Copy className="h-3 w-3" />
      <span>{copied ? intl.formatMessage(i18n.copied) : intl.formatMessage(i18n.copy)}</span>
    </button>
  );
}
