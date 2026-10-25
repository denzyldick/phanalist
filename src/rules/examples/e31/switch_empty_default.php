<?php

function empty_default_falls_through_switch(int $value): string
{
    switch ($value) {
        case 1:
            return 'one';
        default:
    }

    return 'other';
}

function nested_array_write_without_false_key(bool $cafile): string
{
    $options = ['ssl' => []];

    if ($cafile) {
        $options['ssl']['cafile'] = 'ca.pem';
    } else {
        $options['ssl']['capath'] = '/etc/ssl';
    }

    return $options['ssl']['cafile'] ?? '';
}

function closure_by_reference_mutation(): string
{
    $observed = [];

    $record = function () use (&$observed): void {
        $observed['value'] = 'present';
    };

    $record();

    return $observed['value'];
}
