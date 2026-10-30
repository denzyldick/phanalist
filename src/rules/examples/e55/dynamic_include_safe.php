<?php

function load(): void
{
    include 'bootstrap.php';
    include_once __DIR__ . '/bootstrap.php';
    require APP_ROOT . '/config.php';
    require_once __FILE__;
    require_once dirname(__DIR__) . '/config.php';
    require_once dirname(__DIR__, 2) . '/vendor/autoload.php';
    require_once realpath(__DIR__ . '/..') . '/bootstrap.php';
}
