import { Button } from '@/client/ui/button.js';
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogPanel, DialogTitle } from '@/client/ui/dialog.js';
import { Field, FieldDescription, FieldLabel } from '@/client/ui/field.js';
import { Fieldset, FieldsetLegend } from '@/client/ui/fieldset.js';
import { Input } from '@/client/ui/input.js';
import { Label } from '@/client/ui/label.js';
import { Radio, RadioGroup } from '@/client/ui/radio-group.js';
import { Textarea } from '@/client/ui/textarea.js';
import { useEffect, useRef, useState } from 'react';
import { useStore } from '@/client/store.js';
import type { FollowRefs } from '@/shared/protocol.js';

const FOLLOW_REFS: [FollowRefs, string][] = [
  ['on', 'Always'],
  ['auto', 'Auto (not on branches)'],
  ['off', 'Never'],
];

export function SettingsDialog({ onClose }: { onClose: () => void }) {
  const config = useStore((s) => s.config);
  const saveConfig = useStore((s) => s.saveConfig);
  const [text, setText] = useState(config.autoViewed.join('\n'));
  const [context, setContext] = useState(String(config.contextLines));
  const [followRefs, setFollowRefs] = useState(config.followRefs);
  const [saving, setSaving] = useState(false);
  const overrides = Object.entries(config.lspCommands).sort(([a], [b]) => a.localeCompare(b));

  const save = async () => {
    setSaving(true);
    await saveConfig({
      autoViewed: text
        .split('\n')
        .map((l) => l.trim())
        .filter(Boolean),
      contextLines: Math.max(0, Number.parseInt(context, 10) || 0),
      followRefs,
    });
    setSaving(false);
    onClose();
  };

  // Escape closes, Enter saves unless the cursor is in the textarea, where it
  // inserts a line. Registered on window in the capture phase so the dialog
  // wins over the global keymap, which also captures Escape on document.
  const latest = useRef({ save, onClose, saving });
  latest.current = { save, onClose, saving };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        latest.current.onClose();
        return;
      }
      if (e.key === 'Enter' && !e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey) {
        if ((e.target as HTMLElement | null)?.tagName === 'TEXTAREA') return;
        e.preventDefault();
        e.stopPropagation();
        if (!latest.current.saving) void latest.current.save();
      }
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, []);

  return (
    <Dialog open onOpenChange={onClose}>
      <DialogContent
        aria-label="Settings"
        showCloseButton={false}
        className="max-w-xl [&_code]:rounded-sm [&_code]:bg-accent [&_code]:px-1 [&_code]:font-mono"
      >
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
        </DialogHeader>
        <DialogPanel className="flex flex-col gap-5">
          <Field>
            <FieldLabel>Auto-viewed patterns</FieldLabel>
            <FieldDescription>
              Matching files start collapsed and viewed. One glob per line: <code>*.lock</code> matches a file name,{' '}
              <code>**/generated/**</code> a path.
            </FieldDescription>
            <Textarea
              className="font-mono text-[0.75rem] [&_textarea]:min-h-40"
              value={text}
              onChange={(e) => setText(e.target.value)}
              spellCheck={false}
              autoFocus
            />
          </Field>
          <Field>
            <FieldLabel>Context lines</FieldLabel>
            <FieldDescription>Unchanged lines around each change.</FieldDescription>
            <Input
              type="number"
              min={0}
              max={10000}
              value={context}
              onChange={(e) => setContext(e.target.value)}
              className="w-28"
            />
          </Field>
          <Fieldset className="flex flex-col gap-2">
            <FieldsetLegend>Reload behavior</FieldsetLegend>
            <p className="m-0 text-xs text-muted-foreground">Reload when a compared ref moves.</p>
            <RadioGroup
              aria-label="Reload behavior"
              className="gap-2"
              value={followRefs}
              onValueChange={(value) => setFollowRefs(value as FollowRefs)}
            >
              {FOLLOW_REFS.map(([value, label]) => (
                <Label key={value} className="cursor-pointer font-normal">
                  <Radio value={value} />
                  {label}
                </Label>
              ))}
            </RadioGroup>
          </Fieldset>
          <Fieldset className="flex flex-col gap-2">
            <FieldsetLegend>Language servers</FieldsetLegend>
            <p className="m-0 text-xs text-muted-foreground">
              <code>diffle lsp</code> lists them; <code>diffle config set-lsp</code> overrides one.
            </p>
            {overrides.length > 0 && (
              <p className="m-0 text-xs text-muted-foreground">
                Overrides:{' '}
                {overrides.map(([language, command], i) => (
                  <span key={language}>
                    {i > 0 && ', '}
                    {language} <code>{command || '(off)'}</code>
                  </span>
                ))}
              </p>
            )}
          </Fieldset>
        </DialogPanel>
        <DialogFooter className="items-center sm:justify-end">
          <span className="flex-1 text-xs text-muted-foreground" title="Installed diffle version">
            diffle v{__DIFFLE_VERSION__}
          </span>
          <Button variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={save} disabled={saving}>
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
