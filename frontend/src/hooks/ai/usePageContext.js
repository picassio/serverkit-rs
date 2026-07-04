import { useMemo } from 'react';
import { useLocation, useParams } from 'react-router-dom';
import { getCoreContext, getSuggestedPrompts } from '../../lib/ai/pageContextMap';

// Resolves the current route into the assistant's page context (label + entity
// ids) plus page-appropriate suggested prompts. Plugin-registered context
// providers are merged on top by AIContext when a message is sent.
export default function usePageContext() {
    const location = useLocation();
    const params = useParams();
    return useMemo(() => {
        const ctx = getCoreContext(location.pathname, params);
        return { ...ctx, suggestedPrompts: getSuggestedPrompts(ctx.entity) };
    }, [location.pathname, params]);
}
