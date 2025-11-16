import { describe, expect, it } from 'vitest';

import { splitTextWithHighlights } from './MiddleEllipsisHighlight';

describe('splitTextWithHighlights', () => {
  it('returns entire string as plain text when no needles are provided', () => {
    expect(splitTextWithHighlights('foo', undefined)).toEqual([
      { text: 'foo', isHighlight: false },
    ]);
    expect(splitTextWithHighlights('foo', [])).toEqual([{ text: 'foo', isHighlight: false }]);
  });

  it('splits around simple highlight matches', () => {
    expect(splitTextWithHighlights('foo bar baz', ['bar'])).toEqual([
      { text: 'foo ', isHighlight: false },
      { text: 'bar', isHighlight: true },
      { text: ' baz', isHighlight: false },
    ]);
  });

  it('merges overlapping highlight spans automatically', () => {
    expect(splitTextWithHighlights('foobarbaz', ['foo', 'ooba', 'bar'])).toEqual([
      { text: 'foobar', isHighlight: true },
      { text: 'baz', isHighlight: false },
    ]);
  });

  it('respects case sensitivity flags', () => {
    expect(splitTextWithHighlights('AlphaBeta', ['alpha'], { caseInsensitive: true })).toEqual([
      { text: 'Alpha', isHighlight: true },
      { text: 'Beta', isHighlight: false },
    ]);

    expect(splitTextWithHighlights('AlphaBeta', ['alpha'], { caseInsensitive: false })).toEqual([
      { text: 'AlphaBeta', isHighlight: false },
    ]);
  });

  it('handles multiple non-overlapping matches', () => {
    expect(splitTextWithHighlights('abc abc abc', ['abc'])).toEqual([
      { text: 'abc', isHighlight: true },
      { text: ' ', isHighlight: false },
      { text: 'abc', isHighlight: true },
      { text: ' ', isHighlight: false },
      { text: 'abc', isHighlight: true },
    ]);
  });
});
