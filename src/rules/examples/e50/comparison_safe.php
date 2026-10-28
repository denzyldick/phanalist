<?php

function contains(string $haystack, string $needle): bool
{
    if (strpos($haystack, $needle) !== false) {
        return true;
    }

    if (strpos($haystack, $needle) === 0) {
        return true;
    }

    if (($position = strpos($haystack, $needle)) !== false && $position > 0) {
        return true;
    }

    if (strrpos($haystack, $needle) >= 0) {
        return true;
    }

    return false;
}
