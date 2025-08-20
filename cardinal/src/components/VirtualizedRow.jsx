import React from 'react';
import { formatKB } from '../utils/format';

export function VirtualizedRow({ item, index, style }) {
  const path = typeof item === 'string' ? item : item?.path;
  const mtimeSec = typeof item !== 'string' ? (item?.metadata?.mtime ?? item?.mtime) : undefined;
  const mtimeText = mtimeSec != null ? new Date(mtimeSec * 1000).toLocaleString() : null;
  const ctimeSec = typeof item !== 'string' ? (item?.metadata?.ctime ?? item?.ctime) : undefined;
  const ctimeText = ctimeSec != null ? new Date(ctimeSec * 1000).toLocaleString() : null;
  const sizeBytes = typeof item !== 'string' ? (item?.metadata?.size ?? item?.size) : undefined;
  const sizeText = formatKB(sizeBytes);
  return (
    <div
      style={style}
      className={`row ${index % 2 === 0 ? 'row-even' : 'row-odd'}`}
    >
      {item ? (
        <div className="columns row-inner" title={path}>
          <span className="path-text">{path}</span>
          {mtimeText ? (
            <span className="mtime-text">{mtimeText}</span>
          ) : (
            <span className="mtime-text muted">—</span>
          )}
          {ctimeText ? (
            <span className="ctime-text">{ctimeText}</span>
          ) : (
            <span className="ctime-text muted">—</span>
          )}
          {sizeText ? (
            <span className="size-text">{sizeText}</span>
          ) : (
            <span className="size-text muted">—</span>
          )}
        </div>
      ) : (
        <div />
      )}
    </div>
  );
}
