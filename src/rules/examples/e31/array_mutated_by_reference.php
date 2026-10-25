<?php

function array_mutated_by_reference(string $subject): string
{
    $matches = [];

    preg_match('/foo/', $subject, $matches);

    return $matches[0] ?? '';
}
