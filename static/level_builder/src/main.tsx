import { render } from 'preact';
import { App } from './app';
import './styles/base.css';
import './styles/layout.css';
import './styles/components.css';
import './styles/canvas.css';
import './styles/font-picker.css';

render(<App />, document.getElementById('app')!);
