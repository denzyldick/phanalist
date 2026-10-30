<?php

final class Token
{
    public int $id;
}

function mutate(array $tokens): void
{
    foreach ($tokens as $token) {
        $token->id = 1;
    }
}
