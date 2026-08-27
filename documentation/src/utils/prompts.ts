import type { Prompt } from '@site/src/types/prompt';

type PromptModule = Prompt | { default: Prompt };

type RequireContext = {
  (key: string): PromptModule;
  keys(): string[];
};

const webpackRequire = require as typeof require & {
  context(directory: string, useSubdirectories: boolean, pattern: RegExp): RequireContext;
};

const promptContext = webpackRequire.context(
  '../pages/prompt-library/data/prompts',
  false, 
  /\.json$/
);

// Convert the modules into an array of prompts
const prompts: Prompt[] = promptContext.keys().map((key) => {
  const prompt = promptContext(key);
  return 'default' in prompt ? prompt.default : prompt;
});

export async function searchPrompts(query: string): Promise<Prompt[]> {
  const searchTerms = query.toLowerCase().split(' ').filter(Boolean);
  
  if (!searchTerms.length) {
    return prompts;
  }

  return prompts.filter((prompt) => {
    const searchableText = [
      prompt.title,
      prompt.description,
      prompt.example_prompt,
      ...prompt.extensions.map(ext => ext.name)
    ].join(' ').toLowerCase();

    return searchTerms.every(term => searchableText.includes(term));
  });
}

export async function getPromptById(id: string): Promise<Prompt | null> {
  return prompts.find(prompt => prompt.id === id) || null;
}
