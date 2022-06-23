import {
  Button,
  Heading,
  Menu,
  MenuButton,
  MenuDivider,
  MenuGroup,
  MenuItem,
  MenuList,
  useColorModeValue,
} from '@chakra-ui/react';
import { useRouter } from 'next/router';
import * as React from 'react';
import { primaryButtonStyles } from '../common/ button_styles';

export const UserMenu = ({ name }: { name: string }) => {
  const router = useRouter();
  const logoutAction = async () => {
    const { ok } = await fetch('http://localhost:3000/logout', {
      credentials: 'include',
    });
    if (ok) {
      router.reload();
    }
  };

  return (
    <Menu autoSelect={false}>
      <MenuButton
        as={Heading}
        size="s"
        textAlign={'center'}
        display={'flex'}
        noOfLines={1}
        maxWidth={'240px'}
        colorScheme="orange"
      >
        {name}
      </MenuButton>
      <MenuList
        minWidth="240px"
        bg={useColorModeValue('white', 'gray.800')}
        border={'0'}
        boxShadow={'xl'}
        minW={'sm'}
      >
        <MenuGroup title="Profile">
          <MenuItem
            value="profile"
            {...primaryButtonStyles}
            _hover={{ textColor: 'orange.400', bg: useColorModeValue('orange.50', 'gray.900') }}
          >
            My Profile
          </MenuItem>
          <MenuItem
            value="recipes"
            {...primaryButtonStyles}
            _hover={{ textColor: 'orange.400', bg: useColorModeValue('orange.50', 'gray.900') }}
          >
            My Recipes
          </MenuItem>
          <MenuItem
            value="settings"
            {...primaryButtonStyles}
            _hover={{ textColor: 'orange.400', bg: useColorModeValue('orange.50', 'gray.900') }}
          >
            Settings
          </MenuItem>
        </MenuGroup>
        <MenuDivider />
        <MenuGroup>
          <MenuItem
            onClick={logoutAction}
            value="logout"
            _hover={{ textColor: 'orange.400', bg: useColorModeValue('orange.50', 'gray.900') }}
            {...primaryButtonStyles}
          >
            Logout
          </MenuItem>
        </MenuGroup>
      </MenuList>
    </Menu>
  );
};