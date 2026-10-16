import { Button } from '@/client/ui/button.js';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogPanel,
  DialogTitle,
} from '@/client/ui/dialog.js';
import { Textarea } from '@/client/ui/textarea.js';

/** Shown when the browser blocks clipboard access: the text to select and copy by hand. */
export function CopyFallbackDialog({ text, onClose }: { text: string; onClose: () => void }) {
  return (
    <Dialog open onOpenChange={onClose}>
      <DialogContent aria-label="Copy manually" showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>Copy manually</DialogTitle>
          <DialogDescription>
            The browser blocked clipboard access. Select the text below and press Ctrl/⌘+c.
          </DialogDescription>
        </DialogHeader>
        <DialogPanel>
          <Textarea
            className="font-mono text-[0.75rem] [&_textarea]:min-h-40"
            readOnly
            value={text}
            autoFocus
            onFocus={(e) => e.currentTarget.select()}
          />
        </DialogPanel>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
