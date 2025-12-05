// Conventional Commits configuration (ADR-133)
// https://www.conventionalcommits.org/
module.exports = {
  extends: ['@commitlint/config-conventional'],
  rules: {
    // Enforce conventional commit types
    'type-enum': [
      2,
      'always',
      [
        'feat',     // New feature
        'fix',      // Bug fix
        'docs',     // Documentation only
        'style',    // Formatting, no code change
        'refactor', // Code change, no feature/fix
        'perf',     // Performance improvement
        'test',     // Adding/updating tests
        'build',    // Build system or dependencies
        'ci',       // CI configuration
        'chore',    // Maintenance tasks
        'revert',   // Revert previous commit
      ],
    ],
    // Require lowercase type
    'type-case': [2, 'always', 'lower-case'],
    // Require type
    'type-empty': [2, 'never'],
    // Subject should not be empty
    'subject-empty': [2, 'never'],
    // Subject should not end with period
    'subject-full-stop': [2, 'never', '.'],
    // Header max length
    'header-max-length': [2, 'always', 100],
  },
};
