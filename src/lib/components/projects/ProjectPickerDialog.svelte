<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog';
  import ProjectSourcePicker from './ProjectSourcePicker.svelte';
  import { inspectorStore } from '$stores/inspector.svelte';

  type PickerStep = 'choose' | 'scanning' | 'review' | 'registering' | 'empty' | 'error';

  const store = inspectorStore;
  let pickerStep = $state<PickerStep>('choose');
  const headerCopy = $derived(
    pickerStep === 'review'
      ? {
          title: 'Choose projects',
          description: 'Select the repositories you want VibeInspector to register.',
        }
      : pickerStep === 'registering'
        ? {
            title: 'Adding projects',
            description: 'Saving the selected repositories to your local workspace.',
          }
        : {
            title: 'Add projects',
            description: 'Find projects from Codex history or choose a folder on this Mac.',
          },
  );
</script>

<Dialog.Root bind:open={store.projectPickerOpen}>
  <Dialog.Content
    class="max-w-[560px] gap-5 p-5"
    onEscapeKeydown={() => store.closeProjectPicker()}
  >
    <Dialog.Header>
      <Dialog.Title>{headerCopy.title}</Dialog.Title>
      <Dialog.Description>{headerCopy.description}</Dialog.Description>
    </Dialog.Header>
    <ProjectSourcePicker
      onComplete={() => store.closeProjectPicker()}
      onStepChange={(step) => (pickerStep = step)}
    />
  </Dialog.Content>
</Dialog.Root>
