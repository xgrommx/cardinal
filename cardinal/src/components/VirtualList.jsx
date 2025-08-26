import React, { useRef, useState, useCallback, useLayoutEffect, useEffect, forwardRef, useImperativeHandle } from 'react';

/**
 * 等高行虚拟列表 (支持任意滚动位置, 不会在底部/中间跳动)
 * Props:
 *  - rowCount: 总行数
 *  - rowHeight: 行高 (px)
 *  - overscan: 额外预渲染的行数 (上/下 各 overscan)
 *  - renderRow(rowIndex, style): 行渲染函数
 *  - onRangeChange(start, end): 当可见+overscan区间变化时回调 (用于数据预加载)
 *  - onScrollSync(scrollLeft): 水平滚动同步 (用于列头同步)
 *  - className: 自定义 class
 * Exposed imperative API via ref:
 *  - scrollToTop()
 *  - scrollToIndex(index, align = 'start')  // 可选: 'start' | 'center' | 'end'
 */
export const VirtualList = forwardRef(function VirtualList({
	rowCount = 0,
	rowHeight = 24,
	overscan = 5,
	renderRow,
	onRangeChange,
	onScrollSync,
	className = '',
	showEmptyState = true
}, ref) {
	const scrollRef = useRef(null);
	const lastScrollLeftRef = useRef(0);
	const rafRef = useRef(0);
	const [range, setRange] = useState({ start: 0, end: -1 });

	const computeRange = useCallback((el) => {
		if (!el) return { start: 0, end: -1 };
		if (rowCount === 0) return { start: 0, end: -1 };
		const viewportHeight = el.clientHeight || 0;
		const rawStart = Math.floor(el.scrollTop / rowHeight);
		const visible = viewportHeight > 0 ? Math.ceil(viewportHeight / rowHeight) : 0;
		const rawEnd = rawStart + visible - 1;
		return {
			start: Math.max(0, rawStart - overscan),
			end: Math.min(rowCount - 1, rawEnd + overscan)
		};
	}, [rowCount, rowHeight, overscan]);

	const updateRange = useCallback(() => {
		const el = scrollRef.current;
		const next = computeRange(el);
		setRange(prev => {
			// 1. 如果计算出的新范围与当前范围相同，则不执行任何操作以避免不必要的重新渲染。
			// 这是性能优化的关键。
			if (prev.start === next.start && prev.end === next.end) return prev;

			// 2. 如果范围确实发生了变化，则触发 onRangeChange 回调。
			// 此回调通常用于父组件，以预先加载新可见范围内项目的数据。
			if (onRangeChange && next.end >= next.start && rowCount > 0) {
				onRangeChange(next.start, next.end);
			}

			// 3. 返回新的范围对象。
			// React 的 useState hook 将检测到状态变化（因为返回了一个新的对象引用），
			// 并安排组件的重新渲染。
			return next;
		});
	}, [computeRange, onRangeChange, rowCount]);

	const handleScroll = useCallback(() => {
		// 使用 requestAnimationFrame 来对滚动事件进行节流，确保滚动处理函数不会在每一帧中执行超过一次。
		// 这对于防止性能瓶颈至关重要。
		if (rafRef.current) cancelAnimationFrame(rafRef.current);
		rafRef.current = requestAnimationFrame(() => {
			// 在下一帧更新渲染的范围
			updateRange();
			const el = scrollRef.current;
			if (!el) return;

			// 同步水平滚动位置，通常用于使外部组件（如列头）与列表的滚动同步。
			const sl = el.scrollLeft;
			if (onScrollSync && sl !== lastScrollLeftRef.current) {
				lastScrollLeftRef.current = sl;
				onScrollSync(sl);
			}
		});
	}, [updateRange, onScrollSync]);

	// 当组件挂载或其尺寸发生变化时，使用 ResizeObserver 来更新渲染范围。
	// 这确保了即使在视口大小动态改变（例如，窗口大小调整）的情况下，列表也能正确显示。
	// useLayoutEffect 用于在 DOM 更新后同步读取布局信息，防止闪烁。
	useLayoutEffect(() => {
		const el = scrollRef.current;
		if (!el) return;
		const ro = new ResizeObserver(() => updateRange());
		ro.observe(el);
		updateRange(); // Initial update
		return () => ro.disconnect();
	}, [rowCount, rowHeight, updateRange]);

	// Recalc when dependencies change explicitly
	useEffect(() => { updateRange(); }, [rowCount, rowHeight, overscan, updateRange]);

	useEffect(() => () => rafRef.current && cancelAnimationFrame(rafRef.current), []);

	useImperativeHandle(ref, () => ({
		scrollToTop: () => {
			const el = scrollRef.current; if (!el) return;
			el.scrollTo({ top: 0, behavior: 'instant' });
			updateRange();
		},
		scrollToIndex: (index, align = 'start') => {
			const el = scrollRef.current; if (!el) return;
			if (index < 0 || index >= rowCount) return;
			const viewportHeight = el.clientHeight || 0;
			const targetTop = index * rowHeight;
			let scrollTop = targetTop;
			if (align === 'center') scrollTop = targetTop - (viewportHeight - rowHeight) / 2;
			else if (align === 'end') scrollTop = targetTop - (viewportHeight - rowHeight);
			scrollTop = Math.max(0, Math.min(scrollTop, rowCount * rowHeight - viewportHeight));
			el.scrollTo({ top: scrollTop });
			updateRange();
		}
	}), [rowCount, rowHeight, updateRange]);

	const { start, end } = range;
	const totalHeight = rowCount * rowHeight;
	const count = end >= start && rowCount > 0 ? end - start + 1 : 0;
	const items = count > 0 ? Array.from({ length: count }, (_, i) => {
		const rowIndex = start + i;
		return renderRow(rowIndex, {
			position: 'absolute',
			top: i * rowHeight,
			height: rowHeight,
			left: 0,
			right: 0
		});
	}) : null;

	return (
		<div
			ref={scrollRef}
			className={className}
			onScroll={handleScroll}
			role="list"
			aria-rowcount={rowCount}
		>
			<div style={{ height: totalHeight, position: 'relative' }}>
				<div className="virtual-list-items" style={{ top: start * rowHeight }}>
					{items}
				</div>
			</div>
			{showEmptyState && rowCount === 0 && (
				<div className="empty-state">
					<div className="empty-icon" aria-hidden="true">
						<svg width="72" height="72" viewBox="0 0 72 72" fill="none" stroke="currentColor" strokeWidth="1.5">
							<circle cx="32" cy="32" r="18" strokeOpacity="0.5" />
							<path d="M45 45 L60 60" strokeLinecap="round" />
							<circle cx="24" cy="30" r="2" fill="currentColor" />
							<circle cx="32" cy="30" r="2" fill="currentColor" />
							<circle cx="40" cy="30" r="2" fill="currentColor" />
							<path d="M25 38 Q32 44 39 38" strokeLinecap="round" strokeLinejoin="round" />
						</svg>
					</div>
					<div className="empty-title">No Results</div>
					<div className="empty-desc">Try adjusting your keywords or filters.</div>
					<ul className="empty-tips">
						<li>Use more specific terms (e.g. src/components)</li>
						<li>Search partial names: part of filename/path</li>
						<li>Case-insensitive by default</li>
					</ul>
				</div>
			)}
		</div>
	);
});

VirtualList.displayName = 'VirtualList';

export default VirtualList;