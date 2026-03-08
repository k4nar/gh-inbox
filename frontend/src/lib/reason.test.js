import { describe, it, expect } from 'vitest';
import { reasonLabel, reasonClass } from './reason.js';

describe('reasonLabel', () => {
  it('maps review_requested to "Review requested"', () => {
    expect(reasonLabel('review_requested')).toBe('Review requested');
  });

  it('maps mention to "Mentioned"', () => {
    expect(reasonLabel('mention')).toBe('Mentioned');
  });

  it('maps assign to "Assigned"', () => {
    expect(reasonLabel('assign')).toBe('Assigned');
  });

  it('returns the raw reason for unknown values', () => {
    expect(reasonLabel('something_unknown')).toBe('something_unknown');
  });
});

describe('reasonClass', () => {
  it('maps review_requested to "review"', () => {
    expect(reasonClass('review_requested')).toBe('review');
  });

  it('maps mention to "mention"', () => {
    expect(reasonClass('mention')).toBe('mention');
  });

  it('maps team_mention to "mention"', () => {
    expect(reasonClass('team_mention')).toBe('mention');
  });

  it('maps assign to "assign"', () => {
    expect(reasonClass('assign')).toBe('assign');
  });

  it('maps unknown reasons to "default"', () => {
    expect(reasonClass('subscribed')).toBe('default');
  });
});
