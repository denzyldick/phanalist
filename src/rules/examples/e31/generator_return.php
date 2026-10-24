<?php

/**
 * @return \Generator<int, string>
 */
function generator_return(): \Generator
{
    yield 1 => 'one';

    return;
}
