<?php

function load(): void
{
    include 'bootstrap.php';
    include_once __DIR__ . '/bootstrap.php';
    require APP_ROOT . '/config.php';
    require_once __FILE__;
}
