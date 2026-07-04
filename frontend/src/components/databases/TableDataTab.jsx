import { useEffect, useState, useCallback } from 'react';
import { ChevronLeft, ChevronRight, RefreshCw, Table2, Columns3, KeyRound, Terminal } from 'lucide-react';
import { runQuery, getStructure, buildBrowseQuery } from './dbAdapter';
import ResultsGrid from './ResultsGrid';

const PAGE_SIZE = 100;

// Browses one table: a paginated data grid (SELECT * LIMIT/OFFSET under the
// hood) plus a Structure view. `rowsEstimate` comes from the tree's table list
// so we can show a range without a COUNT(*) round-trip.
export default function TableDataTab({ conn, tabId, table, rowsEstimate, active, onStatus, onOpenConsole }) {
    const [view, setView] = useState('data');
    const [page, setPage] = useState(0);
    const [data, setData] = useState(null);
    const [dataLoading, setDataLoading] = useState(false);
    const [dataError, setDataError] = useState('');
    const [structure, setStructure] = useState(null);
    const [structureLoading, setStructureLoading] = useState(false);
    const [structureUnsupported, setStructureUnsupported] = useState(false);

    const loadData = useCallback(async (pageIdx) => {
        setDataLoading(true);
        setDataError('');
        try {
            const sql = buildBrowseQuery(conn, table, { limit: PAGE_SIZE, offset: pageIdx * PAGE_SIZE });
            const result = await runQuery(conn, sql, true);
            if (result.success) setData(result);
            else setDataError(result.error || 'Failed to load rows.');
        } catch (err) {
            setDataError(err.message || 'Failed to load rows.');
        } finally {
            setDataLoading(false);
        }
    }, [conn, table]);

    useEffect(() => { loadData(page); }, [loadData, page]);

    async function loadStructure() {
        if (structure || structureLoading || structureUnsupported) return;
        setStructureLoading(true);
        try {
            const result = await getStructure(conn, table);
            if (result.unsupported) setStructureUnsupported(true);
            else if (result.success) setStructure(result.columns || []);
        } catch { /* leave structure empty; the view shows a fallback */ }
        finally { setStructureLoading(false); }
    }

    useEffect(() => {
        if (view === 'structure') loadStructure();
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [view]);

    useEffect(() => {
        if (!active) return;
        const start = rowsEstimate === 0 ? 0 : page * PAGE_SIZE + 1;
        const end = page * PAGE_SIZE + (data?.row_count || 0);
        onStatus?.(tabId, {
            connText: `${conn.dbType} · ${table}`,
            readonly: true,
            rangeText: data ? `${start}–${end}${rowsEstimate != null ? ` of ≈${rowsEstimate}` : ''}` : null,
            execTime: data?.execution_time,
        });
    }, [active, data, page, rowsEstimate, conn, table, onStatus, tabId]);

    const hasMore = (data?.row_count || 0) === PAGE_SIZE;

    return (
        <div className="dbx-table-tab">
            <div className="dbx-table-toolbar">
                <div className="dbx-segmented" role="tablist" aria-label="Table view">
                    <button
                        type="button"
                        role="tab"
                        aria-selected={view === 'data'}
                        className={view === 'data' ? 'is-active' : ''}
                        onClick={() => setView('data')}
                    >
                        <Table2 size={14} aria-hidden="true" /> Data
                    </button>
                    <button
                        type="button"
                        role="tab"
                        aria-selected={view === 'structure'}
                        className={view === 'structure' ? 'is-active' : ''}
                        onClick={() => setView('structure')}
                    >
                        <Columns3 size={14} aria-hidden="true" /> Structure
                    </button>
                </div>

                {view === 'data' && (
                    <div className="dbx-pager">
                        <button
                            type="button"
                            className="dbx-icon-btn"
                            onClick={() => setPage((p) => Math.max(0, p - 1))}
                            disabled={page === 0 || dataLoading}
                            aria-label="Previous page"
                        >
                            <ChevronLeft size={15} aria-hidden="true" />
                        </button>
                        <span className="dbx-pager-label">
                            {rowsEstimate === 0
                                ? '0 rows'
                                : `${page * PAGE_SIZE + 1}–${page * PAGE_SIZE + (data?.row_count || 0)}`}
                            {rowsEstimate != null && rowsEstimate > 0 && <span className="dbx-pager-total"> of ≈{rowsEstimate.toLocaleString()}</span>}
                        </span>
                        <button
                            type="button"
                            className="dbx-icon-btn"
                            onClick={() => setPage((p) => p + 1)}
                            disabled={!hasMore || dataLoading}
                            aria-label="Next page"
                        >
                            <ChevronRight size={15} aria-hidden="true" />
                        </button>
                    </div>
                )}

                <div className="dbx-table-toolbar-spacer" />

                <button type="button" className="dbx-chip" onClick={() => onOpenConsole?.(`SELECT * FROM ${table} LIMIT 100;`)}>
                    <Terminal size={14} aria-hidden="true" /> Query
                </button>
                <button
                    type="button"
                    className="dbx-icon-btn"
                    onClick={() => (view === 'data' ? loadData(page) : (setStructure(null), loadStructure()))}
                    disabled={dataLoading || structureLoading}
                    aria-label="Refresh"
                >
                    <RefreshCw size={14} className={dataLoading || structureLoading ? 'dbx-spin' : ''} aria-hidden="true" />
                </button>
            </div>

            <div className="dbx-table-body">
                {view === 'data' ? (
                    <ResultsGrid
                        columns={data?.columns}
                        rows={data?.rows}
                        loading={dataLoading}
                        error={dataError}
                        emptyTitle="Empty table"
                        emptyDescription="This table has no rows yet."
                    />
                ) : structureUnsupported ? (
                    <div className="dbx-structure-fallback">
                        Column metadata isn&apos;t available for databases inside Docker containers. Use the Data view or a SQL console.
                    </div>
                ) : structureLoading ? (
                    <div className="dbx-structure-fallback">Loading structure…</div>
                ) : (
                    <div className="dbx-grid-scroll">
                        <table className="dbx-grid dbx-structure">
                            <thead>
                                <tr>
                                    <th scope="col">Column</th>
                                    <th scope="col">Type</th>
                                    <th scope="col">Nullable</th>
                                    <th scope="col">Key</th>
                                    <th scope="col">Default</th>
                                </tr>
                            </thead>
                            <tbody>
                                {(structure || []).map((col) => {
                                    const isPk = col.key === 'PRI' || col.primary_key;
                                    return (
                                        <tr key={col.name}>
                                            <td className="dbx-col-name">
                                                {isPk && <KeyRound size={12} className="dbx-pk" aria-label="Primary key" />}
                                                {col.name}
                                            </td>
                                            <td className="dbx-mono">{col.type}</td>
                                            <td>{col.nullable ? 'YES' : 'NO'}</td>
                                            <td className="dbx-mono">{isPk ? 'PRIMARY' : (col.key || '—')}</td>
                                            <td className={col.default == null ? 'is-null dbx-mono' : 'dbx-mono'}>
                                                {col.default == null ? 'NULL' : String(col.default)}
                                            </td>
                                        </tr>
                                    );
                                })}
                                {structure && structure.length === 0 && (
                                    <tr><td colSpan={5} className="dbx-structure-fallback">No columns reported.</td></tr>
                                )}
                            </tbody>
                        </table>
                    </div>
                )}
            </div>
        </div>
    );
}
