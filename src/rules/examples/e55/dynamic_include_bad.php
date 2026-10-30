<?php

function load(string $name, string $base): void
{
    include $name;
    include_once $base . '/bootstrap.php';
    require $name . '.php';
    require_once dirname(__DIR__) . '/' . $name . '.php';
}
