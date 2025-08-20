import React from 'react';

export function ColumnHeader({ colWidths, onResizeStart }) {
  return (
    <div className="header-row columns">
      <span className="path-text header header-cell">
        Path
        <span className="col-resizer" onMouseDown={onResizeStart('path')} />
      </span>
      <span className="mtime-text header header-cell">
        Modified
        <span className="col-resizer" onMouseDown={onResizeStart('modified')} />
      </span>
      <span className="ctime-text header header-cell">
        Created
        <span className="col-resizer" onMouseDown={onResizeStart('created')} />
      </span>
      <span className="size-text header header-cell">
        Size
        <span className="col-resizer" onMouseDown={onResizeStart('size')} />
      </span>
    </div>
  );
}
