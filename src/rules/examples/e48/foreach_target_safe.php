<?php

function walk($node): void
{
    foreach ($node->children as $node) {
        walk($node);
    }
}
