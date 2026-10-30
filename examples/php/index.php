<?php

declare(strict_types=1);

require __DIR__ . '/vendor/autoload.php';

$dsn = getenv('SENTRY_DSN') ?: ($argv[1] ?? '');
if ($dsn === '') {
    fwrite(STDERR, "Set SENTRY_DSN to the project DSN from the Owlee UI.\n");
    exit(1);
}

\Sentry\init([
    'dsn' => $dsn,
    'environment' => 'production',
    'release' => 'php-example@1.0.0',
    'send_default_pii' => false,
]);

\Sentry\configureScope(function (\Sentry\State\Scope $scope): void {
    $scope->setUser([
        'id' => '42',
        'email' => 'ada@example.com',
        'username' => 'ada',
    ]);
    $scope->setTag('example', 'php');
    $scope->setTag('service', 'checkout');
});

\Sentry\addBreadcrumb(
    category: 'example',
    message: 'about to capture a handled exception',
    level: \Sentry\Breadcrumb::LEVEL_INFO,
);

try {
    throw new RuntimeException('Manual capture from the PHP example');
} catch (Throwable $exception) {
    \Sentry\captureException($exception);
}

// The SDK's exception handler reports this during shutdown, then the process exits.
throw new InvalidArgumentException('Uncaught exception from the PHP example');
