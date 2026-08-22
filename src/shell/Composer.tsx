import type { ReactNode } from "react";

export function Composer({
  quote,
  onClearQuote,
  tools,
  textarea,
  hint,
}: {
  quote: { author: string; excerpt: string } | null;
  onClearQuote: () => void;
  tools: ReactNode;
  textarea: ReactNode;
  hint: ReactNode;
}) {
  return (
    <div className="wp-composer-wrap">
      {quote && (
        <div className="wp-quote-bar">
          <span>引用 {quote.author}：{quote.excerpt}</span>
          <button type="button" aria-label="取消引用" onClick={onClearQuote}>×</button>
        </div>
      )}
      <div className="wp-composer">
        <div className="wp-composer-tools">{tools}</div>
        {textarea}
        <div className="wp-composer-hint">{hint}</div>
      </div>
    </div>
  );
}
