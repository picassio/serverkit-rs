import { useState, useRef, useEffect, useCallback, useMemo } from 'react';
import { NavLink, useLocation } from 'react-router-dom';
import { MoreHorizontal } from 'lucide-react';
import { Popover, PopoverTrigger, PopoverContent } from '@/components/ui/popover';
import { cn } from '@/lib/utils';
import { useOverflowItems } from '@/hooks/useOverflowItems';

// The demo's page top bar (see docs/REDESIGN_MAP.md §6 decision 3): infra pages
// carry their own top bar — an icon + title, an optional routed sub-nav that
// replaces sidebar sub-menus, a spacer, and right-aligned actions.
//
//   <PageTopbar icon={<Globe/>} title="Domains"
//       tabs={[{ to:'/domains', label:'Domains', end:true }, { to:'/dns', label:'DNS Zones' }]}
//       actions={<Button>Add domain</Button>} />
export function PageTopbar({ icon, title, meta, tabs, actions, className }) {
    const hasTabs = tabs && tabs.length > 0;
    return (
        <header className={cn('sk-topbar', className)}>
            {icon && <span className="sk-topbar__ico">{icon}</span>}
            <div className="sk-topbar__titles">
                <h1 className="sk-topbar__title">{title}</h1>
                {meta && <span className="sk-topbar__meta">{meta}</span>}
            </div>

            {/* The tab nav grows to fill the bar; when there are more tabs than
                fit, the overflow collapses into a "More" menu (so groups with
                many sections — e.g. Security — stay on one row). Pages without
                tabs keep the plain spacer that pushes actions to the right. */}
            {hasTabs ? <TopbarTabs tabs={tabs} label={title} /> : <div className="sk-topbar__spacer" />}

            {actions && <div className="sk-topbar__actions">{actions}</div>}
        </header>
    );
}

function matchTab(tab, path) {
    if (tab.end) return path === tab.to;
    // Segment-aware so "/fleet" doesn't swallow "/fleet-monitor".
    return path === tab.to || path.startsWith(tab.to + '/');
}

// Routed sub-nav with overflow handling. Tabs that don't fit the available width
// are hidden and surfaced through a trailing "More" popover via useOverflowItems.
function TopbarTabs({ tabs, label }) {
    const location = useLocation();
    const [popoverOpen, setPopoverOpen] = useState(false);

    const activeIndex = useMemo(
        () => tabs.findIndex((t) => matchTab(t, location.pathname)),
        [tabs, location.pathname]
    );

    const getActiveIndex = useCallback(() => activeIndex, [activeIndex]);

    const { containerRef, itemRefs, moreBtnRef, hiddenIndices, hiddenSet } = useOverflowItems({
        count: tabs.length,
        gap: 2,
        moreWidth: 56,
        getActiveIndex,
        deps: [activeIndex],
    });

    return (
        <nav ref={containerRef} className="sk-topbar__tabs" aria-label={`${label} sections`}>
            {tabs.map((t, i) => {
                const isHidden = hiddenSet.has(i);
                return (
                    <NavLink
                        key={t.to}
                        to={t.to}
                        end={t.end}
                        ref={(el) => { itemRefs.current[i] = el; }}
                        className={({ isActive }) => cn('sk-topbar__tab', isActive && 'is-active')}
                        style={{ display: isHidden ? 'none' : undefined }}
                        data-overflow={isHidden ? 'hidden' : undefined}
                    >
                        {t.icon}
                        {t.label}
                    </NavLink>
                );
            })}
            {hiddenIndices.length > 0 && (
                <Popover open={popoverOpen} onOpenChange={setPopoverOpen}>
                    <PopoverTrigger asChild>
                        <button
                            ref={moreBtnRef}
                            type="button"
                            className="sk-topbar__tab sk-topbar__more"
                            aria-label="More sections"
                        >
                            <MoreHorizontal size={16} />
                            More
                        </button>
                    </PopoverTrigger>
                    <PopoverContent align="end" sideOffset={6} className="ui-popover-content">
                        <div className="tabs-overflow-list">
                            {hiddenIndices.map((idx) => {
                                const t = tabs[idx];
                                return (
                                    <NavLink
                                        key={t.to}
                                        to={t.to}
                                        end={t.end}
                                        className="tabs-overflow-item"
                                        data-state={idx === activeIndex ? 'active' : 'inactive'}
                                        onClick={() => setPopoverOpen(false)}
                                    >
                                        {t.icon}
                                        {t.label}
                                    </NavLink>
                                );
                            })}
                        </div>
                    </PopoverContent>
                </Popover>
            )}
        </nav>
    );
}

export default PageTopbar;
