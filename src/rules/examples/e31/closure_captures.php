<?php

function read_only_reference_capture_is_not_forgotten(): void
{
    $values = ['present' => true];

    $reader = function () use (&$values): void {
        echo $values['present'];
    };

    echo $values['missing'];
}

function written_reference_capture_is_forgotten(): void
{
    $values = ['present' => true];

    $writer = function () use (&$values): void {
        $values['written'] = true;
    };

    echo $values['missing'];
}
