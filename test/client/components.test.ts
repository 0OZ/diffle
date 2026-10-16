// @vitest-environment jsdom
import { act, createElement, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { ActionButton } from '@/client/components/ActionButton.js';
import { Segment, SegmentedControl } from '@/client/components/SegmentedControl.js';

function button(props: Parameters<typeof ActionButton>[0]): HTMLButtonElement {
  const host = document.createElement('div');
  host.innerHTML = renderToStaticMarkup(createElement(ActionButton, props, 'Delete'));
  return host.querySelector('button')!;
}

describe('ActionButton', () => {
  it('rests as a flat button, with danger only colouring the label', () => {
    const plain = button({});
    expect(plain.hasAttribute('data-feedback')).toBe(false);
    expect(plain.className).not.toContain('bg-primary');
    expect(plain.className).not.toContain('text-destructive-foreground');
    expect(button({ danger: true }).className).toContain('text-destructive-foreground');
  });

  it('arms as the destructive fill when dangerous and the primary fill otherwise', () => {
    const armed = button({ feedback: 'confirm', danger: true });
    expect(armed.dataset.feedback).toBe('confirm');
    expect(armed.className).toContain('bg-destructive');
    expect(armed.className).not.toContain('text-destructive-foreground');
    expect(button({ feedback: 'confirm' }).className).toContain('bg-primary');
  });

  it.each(['posted', 'copied'] as const)('acknowledges %s in the success colour', (feedback) => {
    const done = button({ feedback });
    expect(done.dataset.feedback).toBe(feedback);
    expect(done.className).toContain('bg-success');
  });
});

describe('SegmentedControl', () => {
  async function mount(onChange: (value: string) => void) {
    const host = document.createElement('div');
    document.body.append(host);
    const root = createRoot(host);
    function Harness() {
      const [value, setValue] = useState('split');
      return createElement(
        SegmentedControl<string>,
        {
          value,
          onValueChange: (next) => {
            onChange(next);
            setValue(next);
          },
        },
        createElement(Segment, { value: 'split' }, 'Split'),
        createElement(Segment, { value: 'unified' }, 'Unified'),
      );
    }
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    await act(async () => root.render(createElement(Harness)));
    const pressed = () => [...host.querySelectorAll('button')].filter((b) => b.getAttribute('aria-pressed') === 'true');
    const click = (label: string) =>
      act(async () => [...host.querySelectorAll('button')].find((b) => b.textContent === label)!.click());
    return { pressed, click, unmount: () => act(async () => root.unmount()).then(() => host.remove()) };
  }

  it('moves the selection to the pressed segment', async () => {
    const onChange = vi.fn();
    const view = await mount(onChange);
    expect(view.pressed().map((b) => b.textContent)).toEqual(['Split']);
    await view.click('Unified');
    expect(onChange).toHaveBeenCalledWith('unified');
    expect(view.pressed().map((b) => b.textContent)).toEqual(['Unified']);
    await view.unmount();
  });

  it('keeps the choice when the selected segment is pressed again', async () => {
    const onChange = vi.fn();
    const view = await mount(onChange);
    await view.click('Split');
    expect(onChange).not.toHaveBeenCalled();
    expect(view.pressed().map((b) => b.textContent)).toEqual(['Split']);
    await view.unmount();
  });
});
