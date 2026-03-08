import { render, screen } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import Sidebar from './Sidebar.svelte';

describe('Sidebar', () => {
  it('renders the Inbox nav item', () => {
    render(Sidebar);
    expect(screen.getByText('Inbox')).toBeInTheDocument();
  });

  it('renders the Archived nav item', () => {
    render(Sidebar);
    expect(screen.getByText('Archived')).toBeInTheDocument();
  });

  it('renders the Repositories section label', () => {
    render(Sidebar);
    expect(screen.getByText('Repositories')).toBeInTheDocument();
  });

  it('renders the Codeowner Teams section label', () => {
    render(Sidebar);
    expect(screen.getByText('Codeowner Teams')).toBeInTheDocument();
  });
});
