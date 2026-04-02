import { useState, useEffect, useRef } from 'preact/hooks';
import { allGoogleFonts, fonts } from '../store/state';
import { loadAllGoogleFonts, loadGoogleFont } from '../api/fonts';

interface Props {
  current: string;
  onChange: (font: string) => void;
}

export function FontSelect({ current, onChange }: Props) {
  const [showPicker, setShowPicker] = useState(false);

  if (current) loadGoogleFont(current);
  const style = current ? { fontFamily: `'${current}',system-ui` } : {};

  return (
    <div style="flex:1;position:relative;">
      <button
        class="font-picker-btn"
        style={style}
        onClick={() => setShowPicker(true)}
      >
        {current || '-- none --'}
      </button>
      {showPicker && (
        <FontPickerModal
          onSelect={(v) => {
            onChange(v);
            setShowPicker(false);
          }}
          onClose={() => setShowPicker(false)}
        />
      )}
    </div>
  );
}

interface ModalProps {
  onSelect: (font: string) => void;
  onClose: () => void;
}

function FontPickerModal({ onSelect, onClose }: ModalProps) {
  const [filter, setFilter] = useState('');
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadAllGoogleFonts().then(() => setLoading(false));
  }, []);

  const fontList = allGoogleFonts.value.length > 0 ? allGoogleFonts.value : fonts.value;
  const filtered = filter
    ? fontList.filter(f => f.toLowerCase().includes(filter.toLowerCase()))
    : fontList.slice(0, 120);
  const display = filtered.slice(0, 120);

  return (
    <div
      id="font-picker-overlay"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div class="font-picker-modal">
        <input
          placeholder="Search fonts..."
          autoFocus
          value={filter}
          onInput={(e) => setFilter((e.target as HTMLInputElement).value)}
        />
        <div class="font-list">
          {loading ? (
            <div style="padding:20px;color:#888;">Loading fonts...</div>
          ) : (
            <>
              <div class="font-option" onClick={() => onSelect('')} style="color:#888;">
                -- none --
              </div>
              {display.map(f => {
                loadGoogleFont(f);
                return (
                  <div
                    key={f}
                    class="font-option"
                    onClick={() => onSelect(f)}
                    style={{ fontFamily: `'${f}',system-ui` }}
                  >
                    {f}
                  </div>
                );
              })}
              {filtered.length > 120 && (
                <div style="padding:8px;color:#666;font-size:11px;">
                  Type to search {filtered.length - 120} more...
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
