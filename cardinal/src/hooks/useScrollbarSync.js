import { useEffect } from 'react';

// Custom hook to sync vertical and horizontal scrollbars
export function useScrollbarSync({ listRef, scrollAreaRef, results, colWidths, setVerticalBar, setHorizontalBar }) {
  useEffect(() => {
    function updateVerticalBar() {
      if (!listRef.current || !scrollAreaRef.current) return;
      const grid = listRef.current.Grid || listRef.current;
      const totalRows = results.length;
      const rowHeight = 24;
      const visibleHeight = grid.props.height;
      const totalHeight = totalRows * rowHeight;
      const scrollTop = grid.state ? grid.state.scrollTop : 0;
      if (totalHeight <= visibleHeight) {
        setVerticalBar({ top: 0, height: 0, visible: false });
        return;
      }
      const barHeight = Math.max(32, visibleHeight * visibleHeight / totalHeight);
      const barTop = (scrollTop / totalHeight) * visibleHeight;
      setVerticalBar({ top: barTop, height: barHeight, visible: true });
    }
    function updateHorizontalBar() {
      if (!scrollAreaRef.current) return;
      const el = scrollAreaRef.current;
      const scrollWidth = el.scrollWidth;
      const clientWidth = el.clientWidth;
      const scrollLeft = el.scrollLeft;
      if (scrollWidth <= clientWidth) {
        setHorizontalBar({ left: 0, width: 0, visible: false });
        return;
      }
      const barWidth = Math.max(32, clientWidth * clientWidth / scrollWidth);
      const barLeft = (scrollLeft / scrollWidth) * clientWidth;
      setHorizontalBar({ left: barLeft, width: barWidth, visible: true });
    }
    updateVerticalBar();
    updateHorizontalBar();
    if (!listRef.current) return;
    const grid = listRef.current.Grid || listRef.current;
    const onVScroll = () => updateVerticalBar();
    grid && grid._scrollingContainer && grid._scrollingContainer.addEventListener('scroll', onVScroll);
    const el = scrollAreaRef.current;
    const onHScroll = () => updateHorizontalBar();
    el && el.addEventListener('scroll', onHScroll);
    window.addEventListener('resize', updateHorizontalBar);
    return () => {
      grid && grid._scrollingContainer && grid._scrollingContainer.removeEventListener('scroll', onVScroll);
      el && el.removeEventListener('scroll', onHScroll);
      window.removeEventListener('resize', updateHorizontalBar);
    };
  }, [results, colWidths, listRef, scrollAreaRef, setVerticalBar, setHorizontalBar]);
}
