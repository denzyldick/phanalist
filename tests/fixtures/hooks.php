<?php

class User {
    public string $name {
        get => $this->name;
        set {
            if (strlen($value) === 0) {
                throw new ValueError("Name must be non-empty");
            }
            $this->name = $value;
        }
    }
}
