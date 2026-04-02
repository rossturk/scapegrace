import { Breadcrumb } from './breadcrumb';
import { savePack } from '../store/actions';
import { pack, packVersion, savedVersion } from '../store/state';

function downloadPack() {
  const p = pack.value;
  if (!p) return;
  const json = JSON.stringify(p, null, 2);
  const blob = new Blob([json], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = 'campaigns.json';
  a.click();
  URL.revokeObjectURL(url);
}

export function TitleBar() {
  const dirty = packVersion.value !== savedVersion.value;

  return (
    <div class="title-bar">
      <div class="flex gap-8 items-center">
        <Breadcrumb />
      </div>
      <div class="flex gap-8 items-center">
        {dirty && <span style="font-size:12px;color:#ffa726;">Unsaved changes</span>}
        <button class="primary" onClick={savePack}>Save Pack</button>
        <button onClick={downloadPack}>Download</button>
      </div>
    </div>
  );
}
