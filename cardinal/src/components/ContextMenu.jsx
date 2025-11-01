import React, { useEffect, useRef } from 'react';
import { createPortal } from 'react-dom';

export function ContextMenu({ x, y, items, onClose }) {
  const menuRef = useRef(null);

  useEffect(() => {
    // Close the menu whenever the user clicks anywhere outside the menu surface
    const handleClickOutside = (event) => {
      if (menuRef.current && !menuRef.current.contains(event.target)) {
        onClose();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [onClose]);

  const handleItemClick = (action) => {
    // Invoke the menu action first, then tear down the menu overlay
    action();
    onClose();
  };

  return createPortal(
    <div ref={menuRef} className="context-menu" style={{ top: y, left: x }}>
      <ul>
        {items.map((item, index) => (
          <li key={index} onClick={() => handleItemClick(item.action)}>
            {item.label}
          </li>
        ))}
      </ul>
    </div>,
    document.body,
  );
}
