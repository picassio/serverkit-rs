import EmptyState from './EmptyState';

/**
 * Full-page loader for tab-group pages.
 * Shows a skeleton placeholder inside the standard tab-group inner area.
 */
export function PageLoader({ className = '' }) {
    return (
        <div className={`sk-tabgroup__inner ${className}`.trim()}>
            <EmptyState loading title="Loading" />
        </div>
    );
}

export default PageLoader;
