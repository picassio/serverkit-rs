const TypingIndicator = ({ label }) => (
    <div className="sk-ai-typing" role="status" aria-label="Assistant is thinking">
        {label ? <span className="sk-ai-typing__label">{label}</span> : null}
        <span className="sk-ai-typing__dots" aria-hidden="true">
            <i /><i /><i />
        </span>
    </div>
);

export default TypingIndicator;
