<script lang="ts">
  // Cross-training and rest days are expressed as a free-text note, so a day can
  // hold a note with no workout at all.
  type Props = {
    note: string;
    disabled?: boolean;
    onsave: () => void;
  };

  let { note = $bindable(''), disabled = false, onsave }: Props = $props();

  const MAX_LENGTH = 500;
</script>

<div class="note-field">
  <label for="day-note">Note</label>
  <textarea
    id="day-note"
    bind:value={note}
    maxlength={MAX_LENGTH}
    rows="2"
    placeholder="swim 45min, rest day…"
    {disabled}
  ></textarea>
  <div class="row">
    <span class="count">{note.length}/{MAX_LENGTH}</span>
    <button class="btn-secondary" {disabled} onclick={onsave}>Save note</button>
  </div>
</div>

<style>
  .note-field {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    margin-bottom: 1rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid var(--border);
  }

  label {
    font-size: 0.75rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }

  textarea {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.5rem 0.7rem;
    color: var(--text);
    font: inherit;
    font-size: 0.88rem;
    resize: vertical;
    outline: none;
  }

  textarea:focus { border-color: var(--accent); }
  textarea::placeholder { color: var(--muted); }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .count {
    font-size: 0.72rem;
    color: var(--muted);
  }

  .btn-secondary {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.35rem 0.75rem;
    font: inherit;
    font-size: 0.82rem;
    cursor: pointer;
  }

  .btn-secondary:hover:not(:disabled) { border-color: var(--accent); }
  .btn-secondary:disabled { opacity: 0.5; cursor: default; }
</style>
