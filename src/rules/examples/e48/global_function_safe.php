<?php

namespace App;

function getenv(string $name): string|false
{
    return \getenv($name);
}
