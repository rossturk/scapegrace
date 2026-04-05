import { useState } from 'preact/hooks';
import type { ComponentChildren } from 'preact';

interface Props {
  title: string;
  startOpen?: boolean;
  children: ComponentChildren;
}

export function Collapsible({ title, startOpen = false, children }: Props) {
  const [open, setOpen] = useState(startOpen);

  return (
    <div>
      <div
        class={`collapsible-header ${open ? 'open' : ''}`}
        onClick={() => setOpen(!open)}
      >
        <span>{title}</span>
        <span class="arrow">&#9654;</span>
      </div>
      <div class={`collapsible-body ${open ? '' : 'hidden'}`}>
        {children}
      </div>
    </div>
  );
}
