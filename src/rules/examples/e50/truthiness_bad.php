<?php

function contains(string $haystack, string $needle): bool
{
    if (strpos($haystack, $needle)) {
        return true;
    }

    if (!stripos($haystack, $needle)) {
        return false;
    }

    while (strrpos($haystack, $needle)) {
        $haystack = substr($haystack, 1);
    }

    return strpos($haystack, $needle) && strlen($needle) > 0;
}
