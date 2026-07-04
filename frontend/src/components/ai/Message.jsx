import Markdown from './Markdown';
import ToolCallCard from './ToolCallCard';

const Message = ({ message }) => {
    if (message.role === 'user') {
        return (
            <div className="sk-ai-message sk-ai-message--user">
                <div className="sk-ai-message__bubble">{message.content}</div>
            </div>
        );
    }

    return (
        <div className="sk-ai-message sk-ai-message--assistant">
            {(message.toolCalls || []).map((tc) => (
                <ToolCallCard key={tc.id} call={tc} />
            ))}
            {message.content ? <Markdown text={message.content} /> : null}
            {message.status === 'error' ? (
                <div className="sk-ai-message__error">{message.error || 'Something went wrong.'}</div>
            ) : null}
        </div>
    );
};

export default Message;
