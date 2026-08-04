export default [{
    files: ['extension/**/*.js'],
    languageOptions: {
        ecmaVersion: 'latest',
        sourceType: 'module',
        globals: {
            ARGV: 'readonly',
            TextDecoder: 'readonly',
            global: 'readonly',
            print: 'readonly',
        },
    },
    rules: {
        eqeqeq: 'error',
        'no-constant-condition': 'error',
        'no-redeclare': 'error',
        'no-undef': 'error',
        'no-unreachable': 'error',
        'no-unused-vars': ['error', {argsIgnorePattern: '^_'}],
        'no-var': 'error',
        'object-shorthand': 'error',
        'prefer-const': 'error',
        quotes: ['error', 'single', {avoidEscape: true}],
        semi: ['error', 'always'],
    },
}];
