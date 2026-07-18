<script lang="ts">
  import { FolderSearch, ListChecks } from '@lucide/svelte';
  import ProjectSourcePicker from './ProjectSourcePicker.svelte';

  type PickerStep = 'choose' | 'scanning' | 'review' | 'registering' | 'empty' | 'error';

  let pickerStep = $state<PickerStep>('choose');
  const headerCopy = $derived(
    pickerStep === 'review'
      ? {
          title: 'Choose projects',
          description: 'Select the repositories you want VibeInspector to register.',
        }
      : {
          title: 'Add projects',
          description: 'Find projects from Codex history or choose a folder on this Mac.',
        },
  );
</script>

<main class="onboarding" aria-label="Project setup">
  <section data-step={pickerStep}>
    {#if pickerStep === 'choose' || pickerStep === 'review'}
      <header>
        <span class="section-icon">
          {#if pickerStep === 'review'}
            <ListChecks strokeWidth={1.7} />
          {:else}
            <FolderSearch strokeWidth={1.7} />
          {/if}
        </span>
        <div>
          <h1>{headerCopy.title}</h1>
          <p>{headerCopy.description}</p>
        </div>
      </header>
    {/if}
    <ProjectSourcePicker onStepChange={(step) => (pickerStep = step)} />
  </section>
</main>

<style>
  .onboarding {
    display: grid;
    width: 100%;
    height: 100%;
    min-height: 0;
    background: transparent;
  }

  .onboarding > section {
    width: min(720px, calc(100% - 48px));
    align-self: center;
    justify-self: center;
    padding-bottom: 4vh;
  }

  header {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr);
    align-items: start;
    gap: 12px;
    margin-bottom: 20px;
  }

  .section-icon {
    display: grid;
    width: 28px;
    height: 28px;
    place-items: center;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--muted-foreground);
    background: color-mix(in oklab, var(--muted) 72%, transparent);
    box-shadow: var(--control-shadow);
  }

  .section-icon :global(svg) {
    width: 15px;
  }

  h1 {
    margin: 0;
    font-size: 24px;
    line-height: 1.2;
    font-weight: 650;
    letter-spacing: 0;
  }

  header p {
    max-width: 58ch;
    margin: 5px 0 0;
    color: var(--muted-foreground);
    font-size: 13px;
    line-height: 1.5;
  }
</style>
