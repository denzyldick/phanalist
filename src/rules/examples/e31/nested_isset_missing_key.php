<?php

function nested_isset_missing_key(): void
{
    $values = ['a' => ['b' => 1]];

    if (isset($values['a']['missing'])) {
        echo $values['a']['missing'];
    }

    if (isset($values['a']['b'])) {
        echo $values['a']['b'];
    }
}
