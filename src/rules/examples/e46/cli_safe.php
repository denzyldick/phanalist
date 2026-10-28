<?php

function cli_safe(): void
{
    foreach ($argv as $argument) {
        echo $argument;
    }
}
