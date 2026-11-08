<?php

function load(): void
{
    $name = $_GET['name'] ?? 'default';
    $base = $_GET['base'] ?? '';

    include $name;
    include_once $base . '/bootstrap.php';
    require $name . '.php';
    require_once dirname(__DIR__) . '/' . $name . '.php';
}
