import { Search, X } from 'lucide-react';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

/**
 * Reusable search + filter + sort bar for list pages.
 *
 *   <SearchFilterBar
 *     search={searchTerm}
 *     onSearch={setSearchTerm}
 *     filters={[
 *       { key: 'all', label: 'All' },
 *       { key: 'active', label: 'Active' },
 *     ]}
 *     activeFilter={filter}
 *     onFilterChange={setFilter}
 *   />
 */
export function SearchFilterBar({
    search = '',
    onSearch,
    placeholder = 'Search...',
    filters = [],
    activeFilter,
    onFilterChange,
    sortOptions = [],
    sort,
    onSortChange,
    className,
}) {
    return (
        <div className={cn('sk-search-filter-bar', className)}>
            <div className="sk-search-filter-bar__search">
                <Search size={16} className="sk-search-filter-bar__search-icon" />
                <Input
                    type="text"
                    value={search}
                    onChange={(e) => onSearch?.(e.target.value)}
                    placeholder={placeholder}
                    className="sk-search-filter-bar__input"
                />
                {search && (
                    <Button
                        type="button"
                        size="icon"
                        variant="ghost"
                        className="sk-search-filter-bar__clear"
                        onClick={() => onSearch?.('')}
                        aria-label="Clear search"
                        title="Clear search"
                    >
                        <X size={14} />
                    </Button>
                )}
            </div>

            {filters.length > 0 && (
                <div className="sk-search-filter-bar__filters" role="group" aria-label="Filters">
                    {filters.map((filter) => (
                        <button
                            key={filter.key}
                            type="button"
                            className={cn(
                                'sk-search-filter-bar__chip',
                                activeFilter === filter.key && 'is-active'
                            )}
                            onClick={() => onFilterChange?.(filter.key)}
                        >
                            {filter.label}
                            {filter.count != null && (
                                <span className="sk-search-filter-bar__count">{filter.count}</span>
                            )}
                        </button>
                    ))}
                </div>
            )}

            {sortOptions.length > 0 && (
                <div className="sk-search-filter-bar__sort">
                    <select
                        value={sort}
                        onChange={(e) => onSortChange?.(e.target.value)}
                        aria-label="Sort by"
                    >
                        {sortOptions.map((option) => (
                            <option key={option.key} value={option.key}>{option.label}</option>
                        ))}
                    </select>
                </div>
            )}
        </div>
    );
}

export default SearchFilterBar;
