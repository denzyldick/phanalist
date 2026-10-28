<?php

final class Brackets
{
    public const OPENING = ['(', '[', '{'];
}

function scan(array $characters): void
{
    foreach ($characters as $character) {
        if (in_array($character, Brackets::OPENING)) {
            echo $character;
        }
    }
}
