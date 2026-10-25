<?php

function nested_missing_key_is_reported(): void
{
    $values = ['nested' => []];

    echo $values['nested']['missing'];
}

function nested_write_in_one_branch_does_not_leave_stale_shape(bool $write): void
{
    $values = ['nested' => ['present' => true]];

    if ($write) {
        $values['nested']['written'] = true;
    }

    if (isset($values['nested']['written'])) {
        echo $values['nested']['written'];
    }
}

function nested_by_reference_call_is_forgotten(): void
{
    $values = ['nested' => []];

    fill_nested($values['nested']);

    echo $values['nested']['written'];
}

function fill_nested(array &$values): void
{
    $values['written'] = true;
}
