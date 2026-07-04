import * as React from 'react';
import { Command as CommandPrimitive } from 'cmdk';
import { Search } from 'lucide-react';
import { cn } from '@/lib/utils';
import { Dialog, DialogContent } from '@/components/ui/dialog';

// Styling lives in styles/components/_ui.scss (.ui-command-*).

const Command = React.forwardRef(({ className, ...props }, ref) => (
  <CommandPrimitive ref={ref} className={cn('ui-command', className)} {...props} />
));
Command.displayName = CommandPrimitive.displayName;

function CommandDialog({ children, ...props }) {
  return (
    <Dialog {...props}>
      <DialogContent className="ui-command-dialog">
        <Command>{children}</Command>
      </DialogContent>
    </Dialog>
  );
}

const CommandInput = React.forwardRef(({ className, ...props }, ref) => (
  /* eslint-disable-next-line react/no-unknown-property */
  <div className="ui-command-input-wrapper" cmdk-input-wrapper="">
    <Search className="ui-command-input-icon" />
    <CommandPrimitive.Input
      ref={ref}
      className={cn('ui-command-input', className)}
      {...props}
    />
  </div>
));
CommandInput.displayName = CommandPrimitive.Input.displayName;

const CommandList = React.forwardRef(({ className, ...props }, ref) => (
  <CommandPrimitive.List ref={ref} className={cn('ui-command-list', className)} {...props} />
));
CommandList.displayName = CommandPrimitive.List.displayName;

const CommandEmpty = React.forwardRef((props, ref) => (
  <CommandPrimitive.Empty ref={ref} className="ui-command-empty" {...props} />
));
CommandEmpty.displayName = CommandPrimitive.Empty.displayName;

const CommandGroup = React.forwardRef(({ className, ...props }, ref) => (
  <CommandPrimitive.Group ref={ref} className={cn('ui-command-group', className)} {...props} />
));
CommandGroup.displayName = CommandPrimitive.Group.displayName;

const CommandSeparator = React.forwardRef(({ className, ...props }, ref) => (
  <CommandPrimitive.Separator ref={ref} className={cn('ui-command-separator', className)} {...props} />
));
CommandSeparator.displayName = CommandPrimitive.Separator.displayName;

const CommandItem = React.forwardRef(({ className, ...props }, ref) => (
  <CommandPrimitive.Item ref={ref} className={cn('ui-command-item', className)} {...props} />
));
CommandItem.displayName = CommandPrimitive.Item.displayName;

const CommandShortcut = ({ className, ...props }) => (
  <span className={cn('ui-command-shortcut', className)} {...props} />
);
CommandShortcut.displayName = 'CommandShortcut';

export {
  Command, CommandDialog, CommandInput, CommandList, CommandEmpty,
  CommandGroup, CommandItem, CommandSeparator, CommandShortcut,
};
