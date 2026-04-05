import { useRef, useState } from 'preact/hooks';
import { generateDescription } from '../api/generation';
import { showToast } from './toast';

interface TextareaProps {
  value: string;
  onChange: (val: string) => void;
  context: string;
  rows?: number;
}

export function EnhancedTextarea({ value, onChange, context, rows }: TextareaProps) {
  const ref = useRef<HTMLTextAreaElement>(null);
  const [enhancing, setEnhancing] = useState(false);

  async function enhance() {
    const el = ref.current;
    if (!el || !el.value.trim()) return;
    setEnhancing(true);
    try {
      const isShort = context.includes('one-line');
      const maxLen = isShort ? 'max 12 words, one line' : 'max 5 lines';
      const target = el.value.trim().length < 3
        ? `Generate a short, vivid ${context} (${maxLen})`
        : `Rewrite this ${context} to be more vivid and atmospheric (${maxLen}): ${el.value}`;
      const text = await generateDescription(context || 'roguelike game', target);
      if (text) {
        onChange(text);
        showToast('Enhanced!', 'success');
      }
    } catch (e: any) {
      showToast('Enhance failed: ' + e.message, 'error');
    }
    setEnhancing(false);
  }

  return (
    <div class="enhanced-textarea">
      <textarea
        ref={ref}
        rows={rows}
        value={value}
        onChange={(e) => onChange((e.target as HTMLTextAreaElement).value)}
      />
      <button
        class="enhance-btn"
        onClick={enhance}
        disabled={enhancing}
        title="Enhance with AI"
      >
        {enhancing ? <span class="spinner" style="width:10px;height:10px;" /> : '\u2728'}
      </button>
    </div>
  );
}

interface InputProps {
  value: string;
  onChange: (val: string) => void;
  context: string;
  style?: string;
}

export function EnhancedInput({ value, onChange, context, style }: InputProps) {
  const ref = useRef<HTMLInputElement>(null);
  const [enhancing, setEnhancing] = useState(false);

  async function enhance() {
    const el = ref.current;
    if (!el || !el.value.trim()) return;
    setEnhancing(true);
    try {
      const target = el.value.trim().length < 3
        ? `Generate a short, vivid ${context} (max 12 words, one line)`
        : `Rewrite this ${context} to be more vivid and atmospheric (max 12 words, one line): ${el.value}`;
      const text = await generateDescription(context || 'roguelike game', target);
      if (text) {
        onChange(text);
        showToast('Enhanced!', 'success');
      }
    } catch (e: any) {
      showToast('Enhance failed: ' + e.message, 'error');
    }
    setEnhancing(false);
  }

  return (
    <div class="enhanced-textarea">
      <input
        ref={ref}
        value={value}
        style={style}
        onChange={(e) => onChange((e.target as HTMLInputElement).value)}
      />
      <button
        class="enhance-btn"
        onClick={enhance}
        disabled={enhancing}
        title="Enhance with AI"
      >
        {enhancing ? <span class="spinner" style="width:10px;height:10px;" /> : '\u2728'}
      </button>
    </div>
  );
}
